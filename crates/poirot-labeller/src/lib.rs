use database::Database;
use malachite::Rational;
use poirot_metrics::PoirotMetricEvents;
use reth_primitives::{Address, TxHash, U256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::mpsc::UnboundedSender;

pub mod database;

pub struct Metadata {
    pub block_num: u64,
    pub block_hash: U256,
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
    pub token_prices: HashMap<Address, (Rational, Rational)>,
    pub eth_prices: (Rational, Rational),
    pub mempool: HashSet<TxHash>,
}

pub struct Labeller<'a> {
    pub client: &'a Database,
    pub(crate) metrics_tx: UnboundedSender<PoirotMetricEvents>,
}

impl<'a> Labeller<'a> {
    pub fn new(metrics_tx: UnboundedSender<PoirotMetricEvents>, database: &'a Database) -> Self {
        Self { client: database, metrics_tx }
    }

    pub async fn get_metadata(&self, block_num: u64, block_hash: U256) -> Metadata {
        //let res = self.client.query_all::<types::Relay>(&query).await?;
        //println!("{:?}", res);

        todo!()
    }
}
