use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use alloy_providers::provider::Provider;
use alloy_transport_http::Http;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database_libmdbx::Libmdbx;
use brontes_inspect::composer::Composer;
use brontes_pricing::{types::DexPrices, BrontesBatchPricer};
use futures::Future;

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
    provider:      &'db Provider<Http<reqwest::Client>>,
    classifier:    &'db Classifier<'db>,
    dex_price_map: BrontesBatchPricer<T>,

    libmdbx:  &'db Libmdbx,
    composer: Composer<'db, N>,
}

impl<T: TracingProvider, const N: usize> Future for DataBatching<'_, T, N> {
    type Output = HashMap<u64, DexPrices>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
