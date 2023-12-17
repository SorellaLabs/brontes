use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use alloy_providers::provider::Provider;
use alloy_transport_http::Http;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_pricing::{types::DexPrices, BrontesBatchPricer};
use futures::Future;

pub struct DataBatching<'db, T: TracingProvider> {
    parser:        &'db Parser<'db, T>,
    provider:      &'db Provider<Http<reqwest::Client>>,
    classifier:    &'db Classifier<'db>,
    dex_price_map: BrontesBatchPricer,
}

impl<T: TracingProvider> Future for DataBatching<'_, T> {
    type Output = HashMap<u64, DexPrices>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
