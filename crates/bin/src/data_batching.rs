use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::Pair;
use brontes_database_libmdbx::{
    tables::{AddressToProtocol, AddressToTokens},
    Libmdbx,
};
use brontes_inspect::{composer::Composer, Inspector};
use brontes_pricing::{types::DexPrices, BrontesBatchPricer, PairGraph};
use futures::Future;
use reth_db::{cursor::DbCursorRO, transaction::DbTx};

pub struct ResultProcessing<'db, const N: usize> {
    database: &'db Libmdbx,
    composer: Composer<'db, N>,
}

impl<'db, const N: usize> ResultProcessing<'db, N> {
    pub fn new(database: &'db Libmdbx, composer: Composer<'db, N>) -> Self {
        Self { composer, database }
    }
}

pub struct DataBatching<'db, T: TracingProvider, const N: usize> {
    parser:        &'db Parser<'db, T>,
    classifier:    Classifier<'db>,
    dex_price_map: BrontesBatchPricer<T>,

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
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let classifier = Classifier::new(libmdbx, tx);

        let tx = libmdbx.ro_tx().unwrap();
        let binding_tx = libmdbx.ro_tx().unwrap();
        let mut all_addr_to_tokens = tx.cursor_read::<AddressToTokens>().unwrap();
        let mut pairs = HashMap::new();

        for value in all_addr_to_tokens.walk(None).unwrap() {
            if let Ok((address, tokens)) = value {
                let protocol = binding_tx
                    .get::<AddressToProtocol>(address)
                    .unwrap()
                    .unwrap();
                pairs.insert((address, protocol), Pair(tokens.token0, tokens.token1));
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
        Self {
            parser,
            classifier,
            dex_price_map: pricer,
            current_block: start_block,
            end_block,
            libmdbx,
            inspectors,
        }
    }
}

impl<T: TracingProvider, const N: usize> Future for DataBatching<'_, T, N> {
    type Output = HashMap<u64, DexPrices>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
