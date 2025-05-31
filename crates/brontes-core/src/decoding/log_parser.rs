use std::collections::{HashMap, HashSet};

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
use alloy_primitives::{Address, FixedBytes};
#[cfg(feature = "dyn-decode")]
use alloy_primitives::{Address, Log};
use alloy_rpc_types::Log;
#[cfg(feature = "dyn-decode")]
use brontes_types::FastHashMap;
use brontes_types::Protocol;
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::Filter;
#[cfg(feature = "dyn-decode")]
use tracing::info;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
pub struct EthLogParser<T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx: &'static DB,
    pub provider: Arc<T>,
    pub protocol_to_addr_event_vec: HashMap<Protocol, Vec<(Address, FixedBytes<32>)>>,
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> Clone for EthLogParser<T, DB> {
    fn clone(&self) -> Self {
        Self {
            libmdbx:                    self.libmdbx,
            provider:                   self.provider.clone(),
            protocol_to_addr_event_vec: self.protocol_to_addr_event_vec.clone(),
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter> EthLogParser<T, DB> {
    pub async fn new(
        libmdbx: &'static DB,
        provider: Arc<T>,
        protocol_to_addr_event_vec: HashMap<Protocol, Vec<(Address, FixedBytes<32>)>>,
    ) -> Self {
        Self { libmdbx, provider, protocol_to_addr_event_vec }
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
    ) -> eyre::Result<HashMap<Protocol, Vec<Log>>> {
        let provider = self.provider.clone();
        let addresses = self
            .protocol_to_addr_event_vec
            .iter()
            .flat_map(|(_, addresses)| addresses.iter().map(|(address, _)| *address))
            .collect::<HashSet<_>>() // Collect into a HashSet to merge duplicates
            .into_iter()
            .collect::<Vec<_>>(); // Convert the HashSet back into a Vec
        let topics = self
            .protocol_to_addr_event_vec
            .iter()
            .flat_map(|(_, events)| events.iter().map(|(_, topic)| *topic))
            .collect::<HashSet<_>>() // Collect into a HashSet to merge duplicates
            .into_iter()
            .collect::<Vec<_>>(); // Convert the HashSet back into a Vec
        let address_to_protocol = self
            .protocol_to_addr_event_vec
            .iter()
            .flat_map(|(protocol, addresses)| {
                addresses
                    .iter()
                    .map(move |(address, _)| (address, protocol))
            })
            .collect::<HashMap<_, _>>();
        let filter = Filter::new()
            .address(addresses)
            .event_signature(topics)
            .from_block(start_block)
            .to_block(end_block);
        tracing::trace!("Getting logs for filter: {:?}", filter);
        let logs = provider.get_logs(&filter).await?;
        if !logs.is_empty() {
            tracing::debug!("Found {} logs", logs.len());
        }
        let mut res: HashMap<Protocol, Vec<Log>> = HashMap::new();
        for log in logs {
            let proto = address_to_protocol
                .get(&log.address())
                .expect("address not found");
            // if Protocol: Copy, `*proto` works; otherwise derive Clone and do
            // `proto.clone()`
            res.entry(**proto).or_default().push(log);
        }
        Ok(res)
    }
}
