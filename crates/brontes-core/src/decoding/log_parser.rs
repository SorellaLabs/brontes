use std::{collections::HashMap, time::Duration};

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
#[cfg(feature = "dyn-decode")]
use alloy_primitives::{Address, Log};
use alloy_primitives::{Address, Log};
#[cfg(feature = "dyn-decode")]
use brontes_types::FastHashMap;
use brontes_types::Protocol;
use futures::future::join_all;
use reth_primitives::BlockHash;
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::{AnyReceiptEnvelope, Filter, Log as RpcLog, TransactionReceipt};
use tracing::error;
#[cfg(feature = "dyn-decode")]
use tracing::info;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
use crate::errors::TraceParseError;
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
        block_num: u64,
    ) -> Option<(u64, HashMap<Protocol, Vec<Log>>)> {
        let provider = self.provider.clone();
        let logs = join_all(self.filters.iter().map(|(protocol, filter)| {
            let provider = provider.clone();
            async move {
                let logs: Vec<Log> = provider
                    .gets_logs(filter)
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
        Some((block_num, logs))
    }
}
