use std::{
    collections::HashMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::MissingDecimals,
};
use brontes_database::{Metadata, MetadataDB};
use brontes_database_libmdbx::Libmdbx;
use brontes_inspect::{composer::Composer, Inspector};
use brontes_pricing::{types::DexQuotes, BrontesBatchPricer, GraphManager};
use brontes_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::BlockTree};
use futures::{stream::FuturesUnordered, Future, FutureExt, Stream, StreamExt};
use reth_primitives::Header;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (BlockTree<Actions>, MetadataDB)> + Send + 'a>>;

pub struct DataBatching<'db, T: TracingProvider, const N: usize> {
    parser:     &'db Parser<'db, T>,
    classifier: Classifier<'db>,

    collection_future: Option<CollectionFut<'db>>,
    pricer:            WaitingForPricerFuture<T>,

    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'db>>>,

    current_block: u64,
    end_block:     u64,
    batch_id:      u64,

    libmdbx:    &'static Libmdbx,
    inspectors: &'db [&'db Box<dyn Inspector>; N],
}

impl<'db, T: TracingProvider, const N: usize> DataBatching<'db, T, N> {
    pub fn new(
        quote_asset: alloy_primitives::Address,
        batch_id: u64,
        start_block: u64,
        end_block: u64,
        parser: &'db Parser<'db, T>,
        libmdbx: &'static Libmdbx,
        inspectors: &'db [&'db Box<dyn Inspector>; N],
    ) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let classifier = Classifier::new(libmdbx, tx);

        let pairs = libmdbx.addresses_inited_before(start_block).unwrap();

        let mut rest_pairs = HashMap::default();
        for i in start_block + 1..end_block {
            let pairs = libmdbx.protocols_created_at_block(i).unwrap_or_default();
            rest_pairs.insert(i, pairs);
        }

        let pair_graph = GraphManager::init_from_db_state(
            pairs,
            HashMap::default(),
            Box::new(|block, pair| libmdbx.try_load_pair_before(block, pair).ok()),
            Box::new(|block, pair, edges| {
                if libmdbx.save_pair_at(block, pair, edges).is_err() {
                    error!("failed to save subgraph to db");
                }
            }),
        );

        let pricer = BrontesBatchPricer::new(
            quote_asset,
            pair_graph,
            rx,
            parser.get_tracer(),
            start_block,
            rest_pairs,
        );

        let pricer = WaitingForPricerFuture::new(pricer);

        Self {
            collection_future: None,
            processing_futures: FuturesUnordered::default(),
            parser,
            classifier,
            pricer,
            current_block: start_block,
            end_block,
            batch_id,
            libmdbx,
            inspectors,
        }
    }

    fn on_parser_resolve(
        meta: MetadataDB,
        traces: Vec<TxTrace>,
        header: Header,
        classifier: Classifier<'db>,
        tracer: Arc<T>,
        libmdbx: &'db Libmdbx,
    ) -> CollectionFut<'db> {
        let (extra, tree) = classifier.build_block_tree(traces, header);
        Box::pin(async move {
            MissingDecimals::new(tracer, libmdbx, extra.tokens_decimal_fill).await;

            (tree, meta)
        })
    }

    fn start_next_block(&mut self) {
        let parser = self.parser.execute(self.current_block);
        let meta = self
            .libmdbx
            .get_metadata_no_dex(self.current_block)
            .unwrap();

        let classifier = self.classifier.clone();

        let fut = Box::pin(parser.then(|x| {
            let (traces, header) = x.unwrap().unwrap();
            Self::on_parser_resolve(
                meta,
                traces,
                header,
                classifier,
                self.parser.get_tracer(),
                self.libmdbx,
            )
        }));

        self.collection_future = Some(fut);
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: Metadata) {
        info!(target:"brontes","dex pricing finished");
        self.processing_futures.push(Box::pin(ResultProcessing::new(
            self.libmdbx,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }
}

impl<T: TracingProvider, const N: usize> Future for DataBatching<'_, T, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // poll pricer
        if let Poll::Ready(Some((tree, meta))) = self.pricer.poll_next_unpin(cx) {
            if meta.block_num == self.end_block {
                info!(
                    batch_id = self.batch_id,
                    end_block = self.end_block,
                    "batch finished completed"
                );
            }

            self.on_price_finish(tree, meta);
        }

        // progress collection future,
        if let Some(mut future) = self.collection_future.take() {
            if let Poll::Ready((tree, meta)) = future.poll_unpin(cx) {
                debug!("built tree");
                let block = self.current_block;
                self.pricer.add_pending_inspection(block, tree, meta);
            } else {
                self.collection_future = Some(future);
            }
        } else if self.current_block != self.end_block {
            self.current_block += 1;
            self.start_next_block();
        }

        // If we have reached end block and there is only 1 pending tree left,
        // send the close message to indicate to the dex pricer that it should
        // return. This will spam it till the pricer closes but this is needed as it
        // could take multiple polls until the pricing is done for the final
        // block.
        if self.pricer.pending_trees.len() <= 1 && self.current_block == self.end_block {
            self.classifier.close();
        }
        // poll insertion
        while let Poll::Ready(Some(_)) = self.processing_futures.poll_next_unpin(cx) {}

        // return condition
        if self.current_block == self.end_block
            && self.collection_future.is_none()
            && self.processing_futures.is_empty()
            && self.pricer.is_done()
        {
            return Poll::Ready(())
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

pub struct WaitingForPricerFuture<T: TracingProvider> {
    handle:        JoinHandle<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>,
    pending_trees: HashMap<u64, (BlockTree<Actions>, MetadataDB)>,
}

impl<T: TracingProvider> WaitingForPricerFuture<T> {
    pub fn new(mut pricer: BrontesBatchPricer<T>) -> Self {
        let future = Box::pin(async move {
            let res = pricer.next().await;
            (pricer, res)
        });

        let handle = tokio::spawn(future);

        Self { handle, pending_trees: HashMap::default() }
    }

    pub fn is_done(&self) -> bool {
        self.pending_trees.is_empty()
    }

    fn resechedule(&mut self, mut pricer: BrontesBatchPricer<T>) {
        let future = Box::pin(async move {
            let res = pricer.next().await;
            (pricer, res)
        });

        let handle = tokio::spawn(future);
        self.handle = handle;
    }

    pub fn add_pending_inspection(
        &mut self,
        block: u64,
        tree: BlockTree<Actions>,
        meta: MetadataDB,
    ) {
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }
}

impl<T: TracingProvider> Stream for WaitingForPricerFuture<T> {
    type Item = (BlockTree<Actions>, Metadata);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(handle) = self.handle.poll_unpin(cx) {
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

// takes the composer + db and will process data and insert it into libmdx
pub struct ResultProcessing<'db, const N: usize> {
    database: &'db Libmdbx,
    composer: Composer<'db, N>,
}

impl<'db, const N: usize> ResultProcessing<'db, N> {
    pub fn new(
        db: &'db Libmdbx,
        inspectors: &'db [&'db Box<dyn Inspector>; N],
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self {
        if let Err(e) = db.insert_quotes(meta_data.block_num, meta_data.dex_quotes.clone()) {
            tracing::error!(err=?e, block_num=meta_data.block_num, "failed to insert dex pricing and state into db");
        }
        let composer = Composer::new(inspectors, tree, meta_data);
        Self { database: db, composer }
    }
}

impl<const N: usize> Future for ResultProcessing<'_, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready((block_details, mev_details)) = self.composer.poll_unpin(cx) {
            info!(
                target:"brontes",
                "Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
                 ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
                 {}\n- Cumulative MEV Priority Fee Paid: {}\n- Builder Address: {:?}\n- Builder \
                 ETH Profit: {}\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer Fee \
                 Recipient: {:?}\n- Proposer MEV Reward: {:?}\n- Proposer Finalized Profit (USD): \
                 {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n",
                block_details.block_number,
                block_details.mev_count,
                block_details.finalized_eth_price,
                block_details.cumulative_gas_used,
                block_details.cumulative_gas_paid,
                block_details.total_bribe,
                block_details.cumulative_mev_priority_fee_paid,
                block_details.builder_address,
                block_details.builder_eth_profit,
                block_details.builder_finalized_profit_usd,
                block_details
                    .proposer_fee_recipient
                    .unwrap_or(Address::ZERO),
                block_details
                    .proposer_mev_reward
                    .map_or("None".to_string(), |v| v.to_string()),
                block_details
                    .proposer_finalized_profit_usd
                    .map_or("None".to_string(), |v| format!("{:.2}", v)),
                block_details.cumulative_mev_finalized_profit_usd
            );

            println!("{mev_details:#?}");
            if self
                .database
                .insert_classified_data(block_details, mev_details)
                .is_err()
            {
                error!("failed to insert classified data into libmdx");
            }

            return Poll::Ready(())
        }
        Poll::Pending
    }
}
