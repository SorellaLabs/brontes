use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_core::decoding::TracingProvider;
use brontes_pricing::BrontesBatchPricer;
use brontes_types::{
    db::{
        dex::DexQuotes,
        metadata::{MetadataCombined, MetadataNoDex},
    },
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{Stream, StreamExt};
use reth_tasks::TaskExecutor;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::info;

pub struct WaitingForPricerFuture<T: TracingProvider> {
    receiver: Receiver<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>,
    tx:       Sender<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>,

    pub(crate) pending_trees: HashMap<u64, (BlockTree<Actions>, MetadataNoDex)>,
    task_executor:            TaskExecutor,
}

impl<T: TracingProvider> WaitingForPricerFuture<T> {
    pub fn new(mut pricer: BrontesBatchPricer<T>, task_executor: TaskExecutor) -> Self {
        let (tx, rx) = channel(2);
        let tx_clone = tx.clone();
        let fut = Box::pin(async move {
            let res = pricer.next().await;
            tx_clone.try_send((pricer, res)).unwrap();
        });

        task_executor.spawn_critical("dex pricer", fut);
        Self { pending_trees: HashMap::default(), task_executor, tx, receiver: rx }
    }

    pub fn is_done(&self) -> bool {
        self.pending_trees.is_empty()
    }

    fn resechedule(&mut self, mut pricer: BrontesBatchPricer<T>) {
        let tx = self.tx.clone();
        let fut = Box::pin(async move {
            let res = pricer.next().await;
            let _ = tx.try_send((pricer, res));
        });

        self.task_executor.spawn_critical("dex pricer", fut);
    }

    pub fn add_pending_inspection(
        &mut self,
        block: u64,
        tree: BlockTree<Actions>,
        meta: MetadataNoDex,
    ) {
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }
}

impl<T: TracingProvider> Stream for WaitingForPricerFuture<T> {
    type Item = (BlockTree<Actions>, MetadataCombined);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(handle) = self.receiver.poll_recv(cx) {
            let (pricer, inner) = handle.unwrap();
            self.resechedule(pricer);

            if let Some((block, prices)) = inner {
                info!(target:"brontes","Collected dex prices for block: {}", block);

                let Some((tree, meta)) = self.pending_trees.remove(&block) else {
                    return Poll::Ready(None)
                };

                let finalized_meta = meta.into_finalized_metadata(prices);
                return Poll::Ready(Some((tree, finalized_meta)))
            } else {
                // means we have completed chunks
                return Poll::Ready(None)
            }
        }

        Poll::Pending
    }
}
