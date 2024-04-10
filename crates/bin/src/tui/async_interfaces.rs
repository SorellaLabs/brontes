use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_types::mev::events::Action;
use crossterm::event::{KeyEvent, MouseEvent};
use futures::{Future, FutureExt, Stream};
use tokio::{
    sync::mpsc::UnboundedReceiver,
    task::{JoinError, JoinHandle},
};

/// for the head component of each page. Each head component should have the tui
/// for updating. they will be polled with a simple should render flag for the
/// page to know if it should update or not
pub trait HeadAsyncComponent {
    fn handle_key_events(&mut self, _key: KeyEvent) {
        return
    }

    fn handle_mouse_events(&mut self, _mouse: MouseEvent) {
        return
    }

    fn poll_with_ctx(&mut self, can_render: bool, cx: &mut Context<'_>) -> Poll<()>;
}

/// For each Sub Component of a head component.
/// the head component will call poll_render to progress the state updates of
/// these sub components, however the head component will decide if the sub
/// component update is worthy of a render
pub trait SubAsyncComponent {
    fn handle_key_events(&mut self, _key: KeyEvent) {
        return
    }

    fn handle_mouse_events(&mut self, _mouse: MouseEvent) {
        return
    }

    /// returns true if the given sub component is ready to be rendered
    fn poll_render(&mut self, cx: &mut Context<'_>) -> Poll<bool>;

    /// will render the given sub components on the given frame with a config to
    /// pass in for specifc sub component rendering
    fn render<C>(&mut self, f: &mut Frame<'_>, area: Rect, config: Option<C>);
}

pub type ComponentFnOutput<T> = Pin<Box<dyn Future<Output = T> + Send + Unpin + Sync + 'static>>;

pub struct ComponentUpdater<I, T>
where
    T: Send + Sync + 'static,
{
    rx:   Pin<Box<dyn Stream<Item = I> + Send + Sync + Unpin>>,
    work: Option<JoinHandle<T>>,
    map:  Box<dyn Fn(I) -> ComponentFnOutput<T> + Send + Sync + Unpin + 'static>,
}

impl<I, T> ComponentUpdater<I, T>
where
    T: Send + Sync + Unpin + 'static,
    I: Send + Sync + Unpin + 'static,
{
    pub fn new(
        rx: Pin<Box<dyn Stream<Item = I> + Send + Sync + Unpin>>,
        map: Box<dyn Fn(I) -> ComponentFnOutput<T> + Send + Sync + Unpin + 'static>,
    ) -> Self {
        Self { rx, map, work: None }
    }

    pub fn change_map_function(
        mut self,

        map: Box<dyn Fn(I) -> ComponentFnOutput<T> + Send + Sync + Unpin + 'static>,
        map: N,
    ) {
        self.map = map;
    }
}

impl<I, T> Stream for ComponentUpdater<I, T>
where
    T: Send + Sync + Unpin + 'static,
    I: Send + Sync + Unpin + 'static,
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
                    Ok(res) => return Poll::Ready(Some(res)),
                    Err(e) => {
                        tracing::error!(%e, "update thread errored");
                        return Poll::Ready(None)
                    }
                }
            } else {
                this.work = Some(handle);
                return Poll::Pending
            }
        }

        if let Poll::Ready(Some(work)) = this.rx.poll_next_unpin(cx) {
            this.work = Some(tokio::spawn((this.map)(work)));
        }

        Poll::Pending
    }
}
