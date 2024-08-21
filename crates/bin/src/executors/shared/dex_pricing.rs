use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use brontes_core::decoding::TracingProvider;
use brontes_pricing::BrontesBatchPricer;
use brontes_types::{
    constants::START_OF_CHAINBOUND_MEMPOOL_DATA,
    db::{dex::DexQuotes, metadata::Metadata},
    normalized_actions::Action,
    tree::BlockTree,
    BrontesTaskExecutor, FastHashMap, FastHashSet,
};
use futures::{Stream, StreamExt};
use tokio::sync::mpsc::{channel, error::TrySendError, Receiver, Sender};
use tracing::{debug, span, Instrument, Level};

pub type PricingReceiver<T> = Receiver<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>;
pub type PricingSender<T> = Sender<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>;

pub struct WaitingForPricerFuture<T: TracingProvider> {
    receiver: PricingReceiver<T>,
    tx:       PricingSender<T>,

    pub(crate) pending_trees: FastHashMap<u64, (BlockTree<Action>, Metadata)>,
    // if metadata fetching fails, we store the block for it here so that we know to not spam load
    // trees and cause memory overflows
    pub tmp_trees:            FastHashSet<u64>,
    task_executor:            BrontesTaskExecutor,
    max_tree_block:           u64,
    pricing_resolved_cache:   VecDeque<(u64, DexQuotes)>,
}

impl<T: TracingProvider> WaitingForPricerFuture<T> {
    pub fn new(pricer: BrontesBatchPricer<T>, task_executor: BrontesTaskExecutor) -> Self {
        let (tx, rx) = channel(100);
        let tx_clone = tx.clone();
        let fut = Box::pin(Self::pricing_thread(pricer, tx_clone));

        task_executor.spawn_critical("dex pricer", fut);
        Self {
            pending_trees: FastHashMap::default(),
            task_executor,
            tx,
            receiver: rx,
            tmp_trees: FastHashSet::default(),
            max_tree_block: 0,
            pricing_resolved_cache: VecDeque::new(),
        }
    }

    async fn pricing_thread(mut pricer: BrontesBatchPricer<T>, tx: PricingSender<T>) {
        let block = pricer.current_block_processing();
        let mut res = pricer
            .next()
            .instrument(span!(Level::ERROR, "Brontes Dex Pricing",
            block_number=%block))
            .await;

        // we will keep trying to send util it is resolved or the channel is dropped
        while let Err(e) = tx.try_send((pricer, res)) {
            let TrySendError::Full((f_pricer, f_res)) = e else { return };

            pricer = f_pricer;
            res = f_res;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub fn pending_trees(&self) -> usize {
        self.tmp_trees.len() + self.pending_trees.len()
    }

    pub fn is_done(&self) -> bool {
        self.pending_trees.is_empty()
    }

    fn reschedule(&mut self, pricer: BrontesBatchPricer<T>) {
        let tx = self.tx.clone();
        let fut = Box::pin(Self::pricing_thread(pricer, tx));

        self.task_executor.spawn_critical("dex pricer", fut);
    }

    pub fn add_failed_tree(&mut self, block: u64) {
        self.tmp_trees.insert(block);
    }

    pub fn add_pending_inspection(&mut self, block: u64, tree: BlockTree<Action>, meta: Metadata) {
        self.max_tree_block = block;
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }

    fn process_resolved_pricing(
        &mut self,
        block: u64,
        prices: DexQuotes,
    ) -> Poll<Option<(BlockTree<Action>, Metadata)>> {
        let Some((mut tree, meta)) = self.pending_trees.remove(&block) else {
            let _ = self.tmp_trees.remove(&block);
            tracing::error!("no tree for price");
            return Poll::Ready(None);
        };

        // try drop trees that we know will never process but be loud about it if there
        // are any. If any, ensure to fix
        self.pending_trees.retain(|pending_block, _| {
            if &block > pending_block {
                tracing::error!(block=%pending_block, "pending tree never had dex pricing");
                return false
            }

            true
        });

        if tree.header.number >= START_OF_CHAINBOUND_MEMPOOL_DATA {
            tree.label_private_txes(&meta);
        }

        let finalized_meta = meta.into_full_metadata(prices);

        return Poll::Ready(Some((tree, finalized_meta)))
    }
}

impl<T: TracingProvider> Stream for WaitingForPricerFuture<T> {
    type Item = (BlockTree<Action>, Metadata);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // check to see if there is cached block
        if !self.pricing_resolved_cache.is_empty() {
            let (resolved_block, pricing) = self.pricing_resolved_cache.pop_front().unwrap();
            if resolved_block <= self.max_tree_block {
                return self.process_resolved_pricing(resolved_block, pricing)
            }

            // not ready yet so push to front
            self.pricing_resolved_cache
                .push_front((resolved_block, pricing));
        }

        if let Poll::Ready(handle) = self.receiver.poll_recv(cx) {
            let Some((pricer, inner)) = handle else {
                tracing::warn!("tokio task exited");
                return Poll::Ready(None)
            };

            self.reschedule(pricer);
            cx.waker().wake_by_ref();

            if let Some((block, prices)) = inner {
                debug!(target:"brontes","Generated dex prices for block: {} ", block);

                if block > self.max_tree_block {
                    tracing::info!(
                        pricing_block=%block,
                        last_metadata_block=%self.max_tree_block,
                        "Pricing completed for block before metadata"
                    );

                    self.pricing_resolved_cache.push_back((block, prices));
                    return Poll::Pending
                }
                return self.process_resolved_pricing(block, prices)
            }

            tracing::info!("pricing returned completed");
            // means we have completed chunks
            return Poll::Ready(None)
        }

        Poll::Pending
    }
}
