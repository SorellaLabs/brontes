use std::collections::HashMap;

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
#[cfg(feature = "dyn-decode")]
use alloy_primitives::{Address, Log};
use alloy_primitives::Log;
#[cfg(feature = "dyn-decode")]
use brontes_types::FastHashMap;
use brontes_types::Protocol;
use futures::future::join_all;
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::{Filter, FilterSet, Topic, ValueOrArray};
use alloy_primitives::Address;
#[cfg(feature = "dyn-decode")]
use tracing::info;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
pub struct EthLogParser<T: LogProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx:      &'static DB,
    pub provider: Arc<T>,
    pub filters:  HashMap<Protocol, Filter>,
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> Clone for EthLogParser<T, DB> {
    fn clone(&self) -> Self {
        Self {
            libmdbx:  self.libmdbx,
            provider: self.provider.clone(),
            filters:  self.filters.clone(),
        }
    }
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> EthLogParser<T, DB> {
    pub async fn new(
        libmdbx: &'static DB,
        provider: Arc<T>,
        filters: HashMap<Protocol, Filter>,
    ) -> Self {
        Self { libmdbx, provider, filters }
    }

    pub async fn best_block_number(&self) -> eyre::Result<u64> {
        self.provider.best_block_number().await
    }

    pub fn get_provider(&self) -> Arc<T> {
        self.provider.clone()
    }

    pub async fn execute_block_discovery(
        self,
        start_block: u64,
        end_block: u64,
    ) -> Option<(u64, HashMap<Protocol, Vec<Log>>)> {
        let provider = self.provider.clone();
        let addresses: Vec<Address> = self.filters.iter().map(|(_, filter)| {
            filter.address.to_value_or_array()
        }).flat_map(|v|{
            match v.as_ref() {
                Some(ValueOrArray::Value(address)) => {
                    vec![address.clone()]
                }
                Some(ValueOrArray::Array(addresses)) => {
                    addresses.clone()
                }
                None => {
                    vec![]
                }
            }
        }).collect::<Vec<_>>();

        let topics: Topic = self.filters.iter().map(|(_, filter)| {
            filter.topics[0].to_value_or_array()
        }).flat_map(|v|{
            match v.as_ref() {
                Some(ValueOrArray::Value(topic)) => {
                    vec![topic.clone()]
                }
                Some(ValueOrArray::Array(topics)) => {
                    topics.clone()
                }
                None => {
                    vec![]
                }
            }
        }).collect::<Vec<_>>().into();

        let filter = Filter::new().address(addresses).event_signature(topics);
        let logs = provider.get_logs(&filter).await?;

        let logs = join_all(self.filters.iter().map(|(protocol, filter)| {
            let provider = provider.clone();
            async move {
                let filter_range = filter.clone().from_block(start_block).to_block(end_block);
                let logs: Vec<Log> = provider
                    .get_logs(filter)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|log| log.inner)
                    .collect();
                (*protocol, logs)
            }
        }))
        .await
        .into_iter()
        .collect::<HashMap<Protocol, Vec<Log>>>();
        Some((end_block, logs))
    }
}
