use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
};

use database::Database;
use malachite::Rational;
use poirot_metrics::PoirotMetricEvents;
use reth_primitives::{Address, TxHash, U256};
use tokio::sync::mpsc::UnboundedSender;

pub mod database;

#[derive(Debug)]
pub struct Metadata {
    pub block_num: u64,
    pub block_hash: U256,
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
    pub token_prices: HashMap<Address, (Rational, Rational)>,
    pub eth_prices: (Rational, Rational),
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward: u64,
    pub mempool: HashSet<TxHash>,
}

impl Metadata {
    pub fn new(
        block_num: u64,
        block_hash: U256,
        relay_timestamp: u64,
        p2p_timestamp: u64,
        token_prices: HashMap<Address, (Rational, Rational)>,
        eth_prices: (Rational, Rational),
        proposer_fee_recipient: Address,
        proposer_mev_reward: u64,
        mempool: HashSet<TxHash>,
    ) -> Self {
        Self {
            block_num,
            block_hash,
            relay_timestamp,
            p2p_timestamp,
            token_prices,
            eth_prices,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool,
        }
    }
}
pub struct Labeller<'a> {
    pub client: &'a Database,
    pub(crate) metrics_tx: UnboundedSender<PoirotMetricEvents>,
}

impl<'a> Labeller<'a> {
    pub fn new(metrics_tx: UnboundedSender<PoirotMetricEvents>, database: &'a Database) -> Self {
        Self { client: database, metrics_tx }
    }

    pub fn get_metadata(
        &self,
        block_num: u64,
        block_hash: U256,
    ) -> Pin<Box<dyn Future<Output = Metadata> + Send + 'a>> {
        Box::pin(self.client.get_metadata(block_num, block_hash))
    }
}
