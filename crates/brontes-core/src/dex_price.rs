use std::{collections::HashMap, pin::Pin, task::Poll};

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_providers::provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport::TransportResult;
use alloy_transport_http::Http;
use brontes_database::database::Database;
use brontes_types::cache_decimals;
use futures::{future::join, join, stream::FuturesUnordered, Future, StreamExt};

pub struct TransactionPoolSwappedTokens {
    pairs:      Vec<(Address, Address)>,
    state_diff: StateDiff,
}

pub trait DexPrice {
    fn get_price(
        &self,
        provider: &Provider<Http<reqwest::Client>>,
        address: Address,
        zto: bool,
        // for overrides
        state_diff: StateDiff,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>>;
}

// we will have a static map for (token0, token1) => Vec<address, exchange type>
// this will then process using async, grab the reserves and process the price.
// and return that with tvl. with this we can calculate weighted price
pub struct DexPricing<'p> {
    provider: &'p Provider<Http<reqwest::Client>>,
}

impl DexPricing {
    pub fn need_prices_for(&mut self, pools_tokens_type: Vec<TransactionPoolSwappedTokens>) {}
}
