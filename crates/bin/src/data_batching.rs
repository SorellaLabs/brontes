use std::{
    collections::HashMap,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::MissingDecimals,
};
use brontes_database::{Metadata, MetadataDB, Pair};
use brontes_database_libmdbx::{
    tables::{AddressToProtocol, AddressToTokens},
    Libmdbx,
};
use brontes_inspect::{composer::Composer, Inspector};
use brontes_pricing::{types::DexPrices, BrontesBatchPricer, PairGraph};
use brontes_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::TimeTree};
use futures::{stream::FuturesUnordered, Future, FutureExt, Stream, StreamExt};
use reth_db::{cursor::DbCursorRO, transaction::DbTx};
use reth_primitives::Header;
use tokio::task::JoinHandle;
use tracing::info;

type CollectionFut<'a> = Pin<Box<dyn Future<Output = (TimeTree<Actions>, MetadataDB)> + Send + 'a>>;

pub struct DataBatching<'db, T: TracingProvider, const N: usize> {
    parser:     &'db Parser<'db, T>,
    classifier: Classifier<'db>,

    collection_future: Option<CollectionFut<'db>>,
    pricer:            WaitingForPricerFuture<T>,

    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'db>>>,

    current_block: u64,
    end_block:     u64,

    libmdbx:    &'db Libmdbx,
    inspectors: &'db [&'db Box<dyn Inspector>; N],
}

impl<'db, T: TracingProvider, const N: usize> DataBatching<'db, T, N> {
    pub fn new(
        quote_asset: alloy_primitives::Address,
        batch_id: u64,
        run: u64,
        start_block: u64,
        end_block: u64,
        parser: &'db Parser<'db, T>,
        libmdbx: &'db Libmdbx,
        inspectors: &'db [&'db Box<dyn Inspector>; N],
    ) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let classifier = Classifier::new(libmdbx, tx);

        let tx = libmdbx.ro_tx().unwrap();
        let binding_tx = libmdbx.ro_tx().unwrap();
        let mut all_addr_to_tokens = tx.cursor_read::<AddressToTokens>().unwrap();
        let mut pairs = HashMap::new();

        for value in all_addr_to_tokens.walk(None).unwrap() {
            if let Ok((address, tokens)) = value {
                if let Ok(Some(protocol)) = binding_tx.get::<AddressToProtocol>(address) {
                    pairs.insert((address, protocol), Pair(tokens.token0, tokens.token1));
                }
            }
        }

        let pair_graph = PairGraph::init_from_hashmap(pairs);

        let pricer = BrontesBatchPricer::new(
            quote_asset,
            run,
            batch_id,
            pair_graph,
            rx,
            parser.get_tracer(),
            start_block,
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
        let (extra, tree) = classifier.build_tree(traces, header);
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

    fn on_price_finish(&mut self, tree: TimeTree<Actions>, meta: Metadata) {
        info!("dex pricing finished");
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
        // progress collection future,
        if let Some(mut future) = self.collection_future.take() {
            if let Poll::Ready((tree, meta)) = future.poll_unpin(cx) {
                info!("built tree");
                let block = self.current_block;
                self.pricer.add_pending_inspection(block, tree, meta);
            } else {
                self.collection_future = Some(future);
            }
        } else {
            if self.current_block == self.end_block
                && self.pricer.is_complete()
                && self.processing_futures.is_empty()
            {
                return Poll::Ready(())
            } else if self.current_block != self.end_block {
                self.current_block += 1;
                self.start_next_block();
            }
        }

        // poll pricer
        while let Poll::Ready(Some((tree, meta))) = self.pricer.poll_next_unpin(cx) {
            self.on_price_finish(tree, meta);
        }

        // poll insertion
        while let Poll::Ready(Some(_)) = self.processing_futures.poll_next_unpin(cx) {}

        if self.current_block == self.end_block
            && self.collection_future.is_none()
            && self.pricer.is_complete()
            && self.processing_futures.is_empty()
        {
            return Poll::Ready(())
        }
        cx.waker().wake_by_ref();

        Poll::Pending
    }
}

pub struct WaitingForPricerFuture<T: TracingProvider> {
    handle:        JoinHandle<(BrontesBatchPricer<T>, Option<(u64, DexPrices)>)>,
    pending_trees: HashMap<u64, (TimeTree<Actions>, MetadataDB)>,
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
        tree: TimeTree<Actions>,
        meta: MetadataDB,
    ) {
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }

    pub fn is_complete(&self) -> bool {
        self.pending_trees.is_empty()
    }
}

impl<T: TracingProvider> Stream for WaitingForPricerFuture<T> {
    type Item = (TimeTree<Actions>, Metadata);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(handle) = self.handle.poll_unpin(cx) {
            let (pricer, inner) = handle.unwrap();
            self.resechedule(pricer);

            if let Some((block, prices)) = inner {
                info!(?block, "got dex prices for block");

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
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self {
        let composer = Composer::new(inspectors, tree, meta_data);
        Self { database: db, composer }
    }
}

impl<const N: usize> Future for ResultProcessing<'_, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready((block_details, mev_details)) = self.composer.poll_unpin(cx) {
            info!(?block_details, "finished processing for block");
            println!("{:#?}", mev_details);
            // self.database
            //     .insert_classified_data(block_details, mev_details);

            return Poll::Ready(())
        }
        Poll::Pending
    }
}
