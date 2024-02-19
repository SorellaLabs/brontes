use std::{collections::HashMap, path, sync::Arc};

#[cfg(feature = "dyn-decode")]
use alloy_json_abi::JsonAbi;
use alloy_primitives::Address;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_metrics::{
    trace::types::{BlockStats, TraceParseErrorKind, TransactionStats},
    PoirotMetricEvents,
};
use brontes_types::{
    db::{builder::BuilderInfo, searcher::SearcherInfo},
    Protocol,
};
use futures::future::join_all;
use reth_primitives::{Header, B256};
#[cfg(feature = "dyn-decode")]
use reth_rpc_types::trace::parity::Action;
use reth_rpc_types::TransactionReceipt;
use serde::{Deserialize, Serialize};
use toml::Table;
use tracing::error;
#[cfg(feature = "dyn-decode")]
use tracing::info;

use super::*;
#[cfg(feature = "dyn-decode")]
use crate::decoding::dyn_decode::decode_input_with_abi;
use crate::errors::TraceParseError;

const CLASSIFIER_CONFIG_FILE_NAME: &str = "config/classifier_config.toml";
const SEARCHER_BUILDER_CONFIG_FILE_NAME: &str = "config/searcher_builder_config.toml";

/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
//#[derive(Clone)]
pub struct TraceParser<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx: &'db DB,
    pub tracer: Arc<T>,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

impl<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> TraceParser<'db, T, DB> {
    pub async fn new(
        libmdbx: &'db DB,
        tracer: Arc<T>,
        metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
    ) -> Self {
        let this = Self {
            libmdbx,
            tracer,
            metrics_tx,
        };
        this.load_classifier_config_data().await;
        this.load_searcher_builder_config_data().await;

        this
    }

    /// loads up the `classifier_config.toml` and ensures the values are in the
    /// database
    async fn load_classifier_config_data(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(CLASSIFIER_CONFIG_FILE_NAME);

        let Ok(config) = toml::from_str::<Table>(&{
            let Ok(path) = std::fs::read_to_string(workspace_dir) else {
                return;
            };
            path
        }) else {
            return;
        };

        for (protocol, inner) in config {
            let protocol: Protocol = protocol.parse().unwrap();
            for (address, table) in inner.as_table().unwrap() {
                let token_addr: Address = address.parse().unwrap();
                let init_block = table.get("init_block").unwrap().as_integer().unwrap() as u64;

                let table: Vec<TokenInfoWithAddressToml> = table
                    .get("token_info")
                    .map(|i| i.clone().try_into())
                    .unwrap_or(Ok(vec![]))
                    .unwrap_or(vec![]);

                for t_info in &table {
                    self.libmdbx
                        .write_token_info(t_info.address, t_info.decimals, t_info.symbol.clone())
                        .await
                        .unwrap();
                }

                let token_addrs = if table.len() < 2 {
                    [Address::default(), Address::default()]
                } else {
                    [table[0].address, table[1].address]
                };

                self.libmdbx
                    .insert_pool(init_block, token_addr, token_addrs, protocol)
                    .await
                    .unwrap();
            }
        }
    }

    async fn load_searcher_builder_config_data(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(SEARCHER_BUILDER_CONFIG_FILE_NAME);

        let config_str =
            std::fs::read_to_string(workspace_dir).expect("Failed to read config file");

        let config: BSConfig = toml::from_str(&config_str).expect("Failed to parse TOML");

        // Process builders
        for (address_str, builder_info) in config.builders {
            let address = address_str.parse().unwrap();

            let existing_info = self.libmdbx.try_fetch_builder_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(builder_info);
                    self.libmdbx
                        .write_builder_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update builder info");
                }
                None => {
                    self.libmdbx
                        .write_builder_info(address, builder_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }

        // Process SearcherEOAs
        for (address_str, searcher_info) in config.searcher_eoas {
            let address = address_str.parse().unwrap();
            let existing_info = self.libmdbx.try_fetch_searcher_eoa_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(searcher_info);
                    self.libmdbx
                        .write_searcher_eoa_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update searcher info");
                }
                None => {
                    self.libmdbx
                        .write_searcher_eoa_info(address, searcher_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }
        // Process SearcherContracts
        for (address_str, searcher_info) in config.searcher_contracts {
            let address = address_str.parse().unwrap();
            let existing_info = self.libmdbx.try_fetch_searcher_contract_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(searcher_info);
                    self.libmdbx
                        .write_searcher_contract_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update searcher info");
                }
                None => {
                    self.libmdbx
                        .write_searcher_contract_info(address, searcher_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }
    }

    pub fn get_tracer(&self) -> Arc<T> {
        self.tracer.clone()
    }

    pub async fn load_block_from_db(&'db self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        let traces = self.libmdbx.load_trace(block_num).ok()?;

        Some((traces, self.tracer.header_by_number(block_num).await.ok()??))
    }

    /// executes the tracing of a given block
    #[allow(unreachable_code)]
    pub async fn execute_block(&'db self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        if let Some(res) = self.load_block_from_db(block_num).await {
            tracing::debug!(%block_num, traces_in_block= res.0.len(),"loaded trace for db");
            return Some(res);
        }
        #[cfg(not(feature = "local-reth"))]
        {
            tracing::error!("no block found in db");
            return None;
        }

        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            #[cfg(feature = "dyn-decode")]
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.2).into())
                .unwrap();
            #[cfg(not(feature = "dyn-decode"))]
            let _ = self
                .metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into());
            return None;
        }
        #[cfg(feature = "dyn-decode")]
        let traces = self
            .fill_metadata(
                parity_trace.0.unwrap(),
                parity_trace.1,
                receipts.0.unwrap(),
                block_num,
            )
            .await;
        #[cfg(not(feature = "dyn-decode"))]
        let traces = self
            .fill_metadata(parity_trace.0.unwrap(), receipts.0.unwrap(), block_num)
            .await;

        let _ = self
            .metrics_tx
            .send(TraceMetricEvent::BlockMetricRecieved(traces.1).into());

        if self
            .libmdbx
            .save_traces(block_num, traces.0.clone())
            .await
            .is_err()
        {
            error!(%block_num, "failed to store traces for block");
        }

        Some((traces.0, traces.2))
    }

    #[cfg(feature = "dyn-decode")]
    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TxTrace>>, HashMap<Address, JsonAbi>, BlockStats) {
        let merged_trace = self
            .tracer
            .replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match merged_trace {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            Err(e) => {
                stats.err = Some((&Into::<TraceParseError>::into(e)).into());
                None
            }
        };

        let json = if let Some(trace) = &trace {
            let addresses = trace
                .iter()
                .flat_map(|t| {
                    t.trace
                        .iter()
                        .filter_map(|inner| match &inner.trace.action {
                            Action::Call(call) => Some(call.to),
                            _ => None,
                        })
                })
                .filter(|addr| self.libmdbx.get_protocol(*addr).is_err())
                .collect::<Vec<Address>>();
            info!("addresses for dyn decoding: {:#?}", addresses);
            //self.libmdbx.get_abis(addresses).await.unwrap()
            HashMap::default()
        } else {
            HashMap::default()
        };

        info!("{:#?}", json);

        (trace, json, stats)
    }

    #[cfg(not(feature = "dyn-decode"))]
    pub(crate) async fn trace_block(&self, block_num: u64) -> (Option<Vec<TxTrace>>, BlockStats) {
        let merged_trace = self
            .tracer
            .replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match merged_trace {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            Err(e) => {
                stats.err = Some((&Into::<TraceParseError>::into(e)).into());
                None
            }
        };

        (trace, stats)
    }

    /// gets the transaction $receipts for a block
    pub(crate) async fn get_receipts(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TransactionReceipt>>, BlockStats) {
        let tx_receipts = self
            .tracer
            .block_receipts(BlockNumberOrTag::Number(block_num))
            .await;
        let mut stats = BlockStats::new(block_num, None);

        let receipts = match tx_receipts {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            _ => None,
        };

        (receipts, stats)
    }

    pub(crate) async fn fill_metadata(
        &self,
        block_trace: Vec<TxTrace>,
        #[cfg(feature = "dyn-decode")] dyn_json: HashMap<Address, JsonAbi>,
        block_receipts: Vec<TransactionReceipt>,
        block_num: u64,
    ) -> (Vec<TxTrace>, BlockStats, Header) {
        let mut stats = BlockStats::new(block_num, None);

        let (traces, tx_stats): (Vec<_>, Vec<_>) =
            join_all(block_trace.into_iter().zip(block_receipts.into_iter()).map(
                |(trace, receipt)| {
                    let tx_hash = trace.tx_hash;

                    self.parse_transaction(
                        trace,
                        #[cfg(feature = "dyn-decode")]
                        &dyn_json,
                        block_num,
                        tx_hash,
                        receipt.transaction_index.try_into().unwrap(),
                        receipt.gas_used.unwrap().to(),
                        receipt.effective_gas_price.to(),
                    )
                },
            ))
            .await
            .into_iter()
            .unzip();

        stats.txs = tx_stats;
        stats.trace();

        (
            traces,
            stats,
            self.tracer
                .header_by_number(block_num)
                .await
                .unwrap()
                .unwrap(),
        )
    }

    /// parses a transaction and gathers the traces
    async fn parse_transaction(
        &self,
        mut tx_trace: TxTrace,
        #[cfg(feature = "dyn-decode")] dyn_json: &HashMap<Address, JsonAbi>,
        block_num: u64,
        tx_hash: B256,
        tx_idx: u64,
        gas_used: u128,
        effective_gas_price: u128,
    ) -> (TxTrace, TransactionStats) {
        let stats = TransactionStats {
            block_num,
            tx_hash,
            tx_idx: tx_idx as u16,
            traces: vec![],
            err: None,
        };

        #[cfg(feature = "dyn-decode")]
        tx_trace.trace.iter_mut().for_each(|iter| {
            let addr = match iter.trace.action {
                Action::Call(ref addr) => addr.to,
                _ => return,
            };

            if let Some(json_abi) = dyn_json.get(&addr) {
                let decoded_calldata = decode_input_with_abi(json_abi, &iter.trace).ok().flatten();
                iter.decoded_data = decoded_calldata;
            }
        });

        tx_trace.effective_price = effective_gas_price;
        tx_trace.gas_used = gas_used;

        (tx_trace, stats)
    }
}

fn workspace_dir() -> path::PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = path::Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

#[derive(Debug, Deserialize, Default)]
pub struct TokenInfoWithAddressToml {
    pub symbol: String,
    pub decimals: u8,
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug)]
struct BSConfig {
    builders: HashMap<String, BuilderInfo>,
    searcher_eoas: HashMap<String, SearcherInfo>,
    searcher_contracts: HashMap<String, SearcherInfo>,
}
