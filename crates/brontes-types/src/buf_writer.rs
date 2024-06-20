use std::{
    pin::{pin, Pin},
    task::{Context, Poll},
};

use alloy_rlp::Encodable;
use bytes::{BufMut, Bytes};
use futures::{stream::Stream, Future, FutureExt, StreamExt};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use tokio::{fs::File, io::AsyncWriteExt};

pub struct DownloadBufWriterWithProgress<S: Stream<Item = Result<Bytes, reqwest::Error>>> {
    progress_bar:    Option<ProgressBar>,
    buffer:          Vec<u8>,
    download_stream: S,
    file:            WriteProgress,
}

impl<S: Stream<Item = Result<Bytes, reqwest::Error>>> DownloadBufWriterWithProgress<S> {
    pub fn new(
        total_download_size: Option<u64>,
        download_stream: S,
        file: File,
        buffer_cap: usize,
    ) -> Self {
        let progress_bar = Self::init_progress_bar(total_download_size);
        Self {
            download_stream,
            progress_bar,
            file: WriteProgress::Idle(file),
            buffer: Vec::with_capacity(buffer_cap),
        }
    }

    fn handle_bytes(&mut self, bytes: Bytes) {
        let rem = self.buffer.remaining_mut();
        let has = bytes.length();

        self.progress_bar
            .as_ref()
            .inspect(|bar| bar.inc(has as u64));

        if has > rem && self.file.can_write() {
            let bytes_to_write = self.buffer.drain(..).chain(bytes).collect::<Vec<u8>>();
            self.file.write(bytes_to_write);
            return
        }

        self.buffer.extend(bytes);
    }

    fn init_progress_bar(total_download_size: Option<u64>) -> Option<ProgressBar> {
        total_download_size.map(|bytes| {
            let progress_bar =
                ProgressBar::with_draw_target(Some(bytes), ProgressDrawTarget::stderr_with_hz(30));
            let style = ProgressStyle::default_bar()
                .template(
                    "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} mb \
                     ({percent}%) | ETA: {eta}",
                )
                .expect("Invalid progress bar template")
                .progress_chars("█>-")
                .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                })
                .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                });
            progress_bar.set_style(style);
            progress_bar.set_message("download progress");

            progress_bar
        })
    }
}

impl<S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin> Future
    for DownloadBufWriterWithProgress<S>
{
    type Output = eyre::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.file.progress(cx);

        loop {
            match this.download_stream.poll_next_unpin(cx) {
                Poll::Ready(Some(bytes)) => match bytes {
                    Ok(bytes) => this.handle_bytes(bytes),
                    Err(e) => return Poll::Ready(Err(e.into())),
                },
                // finished
                Poll::Ready(None) if this.file.can_write() && this.buffer.is_empty() => {
                    return Poll::Ready(Ok(()))
                }
                // not finished but can end
                Poll::Ready(None) if !this.buffer.is_empty() && this.file.can_write() => {
                    let bytes_to_write = this.buffer.drain(..).collect::<Vec<u8>>();
                    this.file.write(bytes_to_write);
                    cx.waker().wake_by_ref();
                }
                // waiting for a prev batch to finish writing
                Poll::Ready(None) if !this.buffer.is_empty() && !this.file.can_write() => {
                    return Poll::Pending
                }
                Poll::Ready(None) | Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub enum WriteProgress {
    Writing(Pin<Box<dyn Future<Output = File> + Send + Unpin + 'static>>),
    Idle(File),
}

impl WriteProgress {
    pub fn can_write(&self) -> bool {
        matches!(self, WriteProgress::Idle(_))
    }

    pub fn write(&mut self, buf: Vec<u8>) {
        assert!(self.can_write(), "tried to write to the pending buffer");

        unsafe {
            let this: Self = std::ptr::read(self as *const _);
            let Self::Idle(mut file_handle) = this else { unreachable!() };

            let fut = Box::pin(async move {
                let buf_moved = buf;
                file_handle.write_all(&buf_moved).await.unwrap();
                file_handle
            }) as Pin<Box<dyn Future<Output = File> + Send + 'static>>;
            let new = Self::Writing(std::mem::transmute(fut));

            std::ptr::write(self, new);
        }
    }

    pub fn progress(&mut self, cx: &mut Context<'_>) {
        if let Self::Writing(handle) = self {
            let mut pinned = pin!(handle);

            if let Poll::Ready(handle) = pinned.poll_unpin(cx) {
                *self = WriteProgress::Idle(handle);
            }
        }
    }
}
