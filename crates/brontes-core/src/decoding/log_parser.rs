use std::collections::HashMap;

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
use alloy_primitives::{Address, FixedBytes};
#[cfg(feature = "dyn-decode")]
use alloy_primitives::{Address, Log};
#[cfg(feature = "dyn-decode")]
use brontes_types::FastHashMap;
use brontes_types::Protocol;
use futures::future::join_all;
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::{Filter, FilterSet, Topic, ValueOrArray};
#[cfg(feature = "dyn-decode")]
use tracing::info;

use alloy_rpc_types::Log;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
pub struct EthLogParser<T: LogProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx:                &'static DB,
    pub provider:           Arc<T>,
    pub protocol_to_events: HashMap<Protocol, (Address, FixedBytes<32>)>,
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> Clone for EthLogParser<T, DB> {
    fn clone(&self) -> Self {
        Self {
            libmdbx:            self.libmdbx,
            provider:           self.provider.clone(),
            protocol_to_events: self.protocol_to_events.clone(),
        }
    }
}

impl<T: LogProvider, DB: LibmdbxReader + DBWriter> EthLogParser<T, DB> {
    pub async fn new(
        libmdbx: &'static DB,
        provider: Arc<T>,
        protocol_to_events: HashMap<Protocol, (Address, FixedBytes<32>)>,
    ) -> Self {
        Self { libmdbx, provider, protocol_to_events }
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

        let addresses = self
            .protocol_to_events
            .iter()
            .map(|(_, (address, _))| address.clone())
            .collect::<Vec<_>>();
        let topics = self
            .protocol_to_events
            .iter()
            .map(|(_, (_, topic))| topic.clone())
            .collect::<Vec<_>>();
        let address_to_protocol = self
            .protocol_to_events
            .iter()
            .map(|(protocol, (address, _))| (address, protocol))
            .collect::<HashMap<_, _>>();

        let filter = Filter::new()
            .address(addresses)
            .event_signature(topics)
            .from_block(start_block)
            .to_block(end_block);
        let logs = provider.get_logs(&filter).await?;
        let res = logs
            .into_iter()
            .fold(HashMap::new(), |mut acc, log| {
                let protocol = address_to_protocol
                    .get(&log.address())
                    .expect("log address not found in protocol_to_events");
                acc.entry(**protocol)
                    .or_insert_with(Vec::new)
                    .push(log);
                acc
            });
        Some((end_block, res))
    }
}
