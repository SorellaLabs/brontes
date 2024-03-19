use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{
    stream::{Fuse, FuturesUnordered, Stream, StreamExt},
    Future,
};

impl<T: ?Sized> BrontesStreamExt for T where T: StreamExt {}

pub trait BrontesStreamExt: StreamExt {
    fn unordered_buffer_map<F, R>(self, size: usize, map: F) -> UnorderedBufferMap<Self, F, R>
    where
        R: Future,
        F: FnMut(Self::Item) -> R,
        Self: Sized,
    {
        UnorderedBufferMap::new(self, map, size)
    }
}

#[pin_project::pin_project]
pub struct UnorderedBufferMap<St, F, R>
where
    St: Stream,
    R: Future,
    F: FnMut(St::Item) -> R,
{
    #[pin]
    stream: Fuse<St>,
    in_progress_queue: FuturesUnordered<R>,
    map: F,
    max: usize,
}

impl<St, F, R> UnorderedBufferMap<St, F, R>
where
    St: Stream,
    R: Future,
    F: FnMut(St::Item) -> R,
{
    pub fn new(stream: St, map: F, max: usize) -> Self {
        Self {
            stream: stream.fuse(),
            in_progress_queue: FuturesUnordered::default(),
            map,
            max,
        }
    }
}

impl<St, F, R> Stream for UnorderedBufferMap<St, F, R>
where
    St: Stream,
    R: Future,
    F: FnMut(St::Item) -> R,
{
    type Item = R::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        // First up, try to spawn off as many futures as possible by filling up
        // our queue of futures.
        while this.in_progress_queue.len() < *this.max {
            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(fut)) => {
                    this.in_progress_queue.push((this.map)(fut));
                }
                Poll::Ready(None) | Poll::Pending => break,
            }
        }
        // Attempt to pull the next value from the in_progress_queue
        match this.in_progress_queue.poll_next_unpin(cx) {
            x @ Poll::Pending | x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {}
        }

        // If more values are still coming from the stream, we're not done yet
        if this.stream.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}
