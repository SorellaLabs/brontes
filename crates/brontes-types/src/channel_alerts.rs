use std::task::{Context, Poll};

use tokio::sync::mpsc::UnboundedReceiver;

pub struct UnboundedYapperReceiver<T> {
    chan:      UnboundedReceiver<T>,
    /// amount of pending in channel to start yappin
    yap_count: usize,
    name:      String,
}

impl<T> UnboundedYapperReceiver<T> {
    pub fn new(chan: UnboundedReceiver<T>, yap_count: usize, name: String) -> Self {
        Self { chan, yap_count, name }
    }

    pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let len = self.chan.len();
        if len > self.yap_count {
            let mb = (std::mem::size_of::<T>() * len) / 1_000_000;
            tracing::warn!(chan=%self.name, mb_usage=mb, "unbounded channel is above threshold");
        }

        self.chan.poll_recv(cx)
    }

    pub async fn recv(&mut self) -> Option<T> {
        let len = self.chan.len();
        if len > self.yap_count {
            let mb = (std::mem::size_of::<T>() * len) / 1_000_000;
            tracing::warn!(chan=%self.name, mb_usage=mb, "unbounded channel is above threshold");
        }

        self.chan.recv().await
    }

    pub fn try_recv(&mut self) -> Result<T, tokio::sync::mpsc::error::TryRecvError> {
        let len = self.chan.len();
        if len > self.yap_count {
            let mb = (std::mem::size_of::<T>() * len) / 1_000_000;
            tracing::warn!(chan=%self.name, mb_usage=mb, "unbounded channel is above threshold");
        }

        self.chan.try_recv()
    }
}
