use std::{pin::Pin, task::Poll};

use brontes_types::mev::events::Action;
use crossterm::event::{KeyEvent, MouseEvent};
use futures::{Future, FutureExt, Stream};
use tokio::task::JoinHandle;

pub trait AsyncComponent: Future<Output = ()> {
    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) -> eyre::Result<Option<Action>> {
        Ok(None)
    }

    #[allow(unused_variables)]
    fn handle_mouse_events(&mut self, mouse: MouseEvent) -> eyre::Result<Option<Action>> {
        Ok(None)
    }
}

use tokio::{sync::mpsc::UnboundedReceiver, task::JoinError};

type ComponentFnOutput<T> =
    Pin<Box<dyn Future<Output = Option<T>> + Send + Unpin + Sync + 'static>>;

pub struct ComponentUpdater<T, F>
where
    F: Fn(Action) -> ComponentFnOutput<T>,
    T: Send + Sync + 'static,
{
    rx:   UnboundedReceiver<Action>,
    work: Option<JoinHandle<Option<T>>>,
    map:  F,
}

impl<T, F> ComponentUpdater<T, F>
where
    F: Fn(Action) -> ComponentFnOutput<T> + Unpin + Send + Sync,
    T: Send + Sync + Unpin + 'static,
{
    pub fn new(rx: UnboundedReceiver<Action>, map: F) -> Self {
        Self { rx, map, work: None }
    }

    pub fn change_map_function<N>(mut self, map: N) -> ComponentUpdater<T, N>
    where
        N: Fn(Action) -> ComponentFnOutput<T> + Unpin + Send + Sync,
    {
        CompComponentUpdater { rx: self.rx, work: self.work, map }
    }
}

impl<T, F> Stream for ComponentUpdater<T, F>
where
    F: Fn(Action) -> ComponentFnOutput<T> + Unpin + Send + Sync,
    T: Send + Sync + Unpin + 'static,
{
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(mut handle) = this.work.take() {
            if let Poll::Ready(res) = handle.poll_unpin(cx) {
                match res {
                    Ok(Some(res)) => return Poll::Ready(Some(res)),
                    Err(e) => {
                        tracing::error!(%e, "update thread errored");
                        return Poll::Ready(None)
                    }
                    _ => {}
                }
            } else {
                this.work = Some(handle);
                return Poll::Pending
            }
        }

        if let Poll::Ready(Some(work)) = this.rx.poll_recv(cx) {
            this.work = Some(tokio::spawn((this.map)(work)));
        }

        Poll::Pending
    }
}
