use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_core::decoding::TracingProvider;
use brontes_pricing::BrontesBatchPricer;
use brontes_types::{
    constants::START_OF_CHAINBOUND_MEMPOOL_DATA,
    db::{
        dex::DexQuotes,
        metadata::Metadata,
        traits::{DBWriter, LibmdbxReader},
    },
    normalized_actions::Action,
    tree::BlockTree,
    BrontesTaskExecutor, FastHashMap,
};
use futures::{Stream, StreamExt};
use tokio::sync::mpsc::{channel, error::TrySendError, Receiver, Sender};
use tracing::{debug, span, Instrument, Level};

pub type PricingReceiver<T, DB> = Receiver<(BrontesBatchPricer<T, DB>, Option<(u64, DexQuotes)>)>;
pub type PricingSender<T, DB> = Sender<(BrontesBatchPricer<T, DB>, Option<(u64, DexQuotes)>)>;

pub struct WaitingForPricerFuture<T: TracingProvider, DB: DBWriter + LibmdbxReader> {
    receiver: PricingReceiver<T, DB>,
    tx:       PricingSender<T, DB>,

    pub(crate) pending_trees: FastHashMap<u64, (BlockTree<Action>, Metadata)>,
    task_executor:            BrontesTaskExecutor,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter + Unpin> Drop
    for WaitingForPricerFuture<T, DB>
{
    fn drop(&mut self) {
        tracing::debug!(rem_trees=?self.pending_trees.len(), keys=?self.pending_trees.keys().collect::<Vec<_>>(), "range has this many pending trees");
        // ensures that we properly drop everything
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let res = self.receiver.recv().await;
                drop(res);
                tracing::debug!("droping pricing future");
            });
        });
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter + Unpin> WaitingForPricerFuture<T, DB> {
    pub fn new(pricer: BrontesBatchPricer<T, DB>, task_executor: BrontesTaskExecutor) -> Self {
        let (tx, rx) = channel(100);
        let tx_clone = tx.clone();
        let fut = Box::pin(Self::pricing_thread(pricer, tx_clone));

        task_executor.spawn_critical("dex pricer", fut);
        Self { pending_trees: FastHashMap::default(), task_executor, tx, receiver: rx }
    }

    async fn pricing_thread(mut pricer: BrontesBatchPricer<T, DB>, tx: PricingSender<T, DB>) {
        let block = pricer.current_block_processing();
        let mut res = pricer
            .next()
            .instrument(span!(Level::ERROR, "Brontes Dex Pricing",
            block_number=%block))
            .await;

        // we will keep trying to send util it is resolved;
        while let Err(e) = tx.try_send((pricer, res)) {
            let TrySendError::Full((f_pricer, f_res)) = e else {
                tracing::error!(err=%e, "failed to send dex pricing result, channel closed");
                return
            };

            pricer = f_pricer;
            res = f_res;
        }
    }

    pub fn is_done(&self) -> bool {
        self.pending_trees.is_empty()
    }

    fn reschedule(&mut self, pricer: BrontesBatchPricer<T, DB>) {
        let tx = self.tx.clone();
        let fut = Box::pin(Self::pricing_thread(pricer, tx));

        self.task_executor.spawn_critical("dex pricer", fut);
    }

    pub fn add_pending_inspection(&mut self, block: u64, tree: BlockTree<Action>, meta: Metadata) {
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader + Unpin> Stream
    for WaitingForPricerFuture<T, DB>
{
    type Item = (BlockTree<Action>, Metadata);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(handle) = self.receiver.poll_recv(cx) {
            let (pricer, inner) = handle.unwrap();
            self.reschedule(pricer);
            cx.waker().wake_by_ref();

            if let Some((block, prices)) = inner {
                debug!(target:"brontes","Generated dex prices for block: {} ", block);

                let Some((mut tree, meta)) = self.pending_trees.remove(&block) else {
                    tracing::error!("no tree for price");
                    return Poll::Ready(None);
                };

                if tree.header.number >= START_OF_CHAINBOUND_MEMPOOL_DATA {
                    tree.label_private_txes(&meta);
                }

                let finalized_meta = meta.into_full_metadata(prices);

                return Poll::Ready(Some((tree, finalized_meta)))
            } else {
                tracing::info!("pricing returned completed");
                // means we have completed chunks
                return Poll::Ready(None)
            }
        }

        Poll::Pending
    }
}
