use crate::{
    decoding::utils::*,
    errors::TraceParseError,
    structured_trace::{
        CallAction,
        StructuredTrace::{self},
        TxTrace,
    },
    *,
};
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_etherscan::Client;
use alloy_json_abi::{JsonAbi, StateMutability};
use colored::Colorize;
use reth_tracing::TracingClient;

use super::*;
use ethers_core::{k256::elliptic_curve::rand_core::block, types::Chain};
use reth_primitives::{H256, U256};
use reth_rpc_types::trace::parity::{
    Action as RethAction, CallAction as RethCallAction, TraceResultsWithTransactionHash,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, error, info, instrument};

/// A [`Parser`] will iterate through a block's Parity traces and attempt to decode each call for
/// later analysis.
#[derive(Debug)]
pub struct Parser {
    pub client: Client,
    pub tracer: TracingClient,
}

impl Parser {
    pub fn new(etherscan_key: String, tracer: TracingClient) -> Self {
        let _paths = fs::read_dir("./").unwrap();

        let _paths = fs::read_dir("./").unwrap_or_else(|err| {
            error!("Failed to read directory: {}", err);
            std::process::exit(1);
        });

        let cache_directory = "./abi_cache";

        // Check if the cache directory exists, and create it if it doesn't.
        if !Path::new(cache_directory).exists() {
            fs::create_dir_all(cache_directory).expect("Failed to create cache directory");
        }

        Self {
            client: Client::new_cached(
                Chain::Mainnet,
                etherscan_key,
                Some(PathBuf::from(cache_directory)),
                CACHE_TIMEOUT,
            )
            .unwrap(),

            tracer,
        }
    }

    // Should parse all transactions, if a tx fails to parse it should still be stored with None
    // fields on the decoded subfield

    #[instrument(skip(self, block_trace))]
    pub async fn parse_block(
        &mut self,
        block_num: u64,
        block_trace: Vec<TraceResultsWithTransactionHash>,
    ) -> Vec<TxTrace> {
        let mut result: Vec<TxTrace> = vec![];

        for (idx, trace) in block_trace.iter().enumerate() {
            // We don't need to through an error for this given transaction so long as the error is
            // logged & emmitted and the transaction is stored.
            init_tx!(trace.transaction_hash, idx, block_trace.len());
            match self.parse_tx(trace, idx, block_num).await {
                Ok(res) => {
                    success_tx!(block_num, trace.transaction_hash);
                    result.push(res);
                }
                Err(e) => {
                    let error: &(dyn std::error::Error + 'static) = &e;
                    error!(error, "Error Parsing Transaction {:#x}", trace.transaction_hash);
                }
            }
        }
        success_block!(block_num);

        result
    }

    pub async fn parse_tx(
        &self,
        trace: &TraceResultsWithTransactionHash,
        tx_index: usize,
        block_num: u64,
    ) -> Result<TxTrace, TraceParseError> {
        let transaction_traces =
            trace.full_trace.trace.as_ref().ok_or(TraceParseError::TraceMissing)?;

        let mut structured_traces = Vec::new();
        let tx_hash = &trace.transaction_hash;

        for (idx, transaction_trace) in transaction_traces.iter().enumerate() {
            init_trace!(tx_hash, idx, transaction_traces.len());

            let (action, trace_address) = if let Some((a, t)) =
                decode_trace_action(&mut structured_traces, &transaction_trace, &tx_hash)
            {
                (a, t)
            } else {
                continue
            };

            let abi = match self.client.contract_abi(action.to.into()).await {
                Ok(a) => a,
                Err(e) => {
                    error_trace!(tx_hash, idx, TraceParseError::from(e));
                    continue
                }
            };

            // Check if the input is empty, indicating a potential `receive` or `fallback` function
            // call.
            if action.input.is_empty() {
                match handle_empty_input(&abi, &action, &trace_address, tx_hash) {
                    Ok(structured_trace) => {
                        structured_traces.push(structured_trace);
                        continue
                    }
                    Err(e) => {
                        error_trace!(tx_hash, idx, e);
                        continue
                    }
                }
            }

            match abi_decoding_pipeline(&self, &abi, &action, &trace_address, &tx_hash, block_num)
                .await
            {
                Ok(s) => {
                    success_trace!(
                        tx_hash,
                        trace_action = "CALL",
                        call_type = format!("{:?}", action.call_type)
                    );
                    structured_traces.push(s);
                }
                Err(e) => {
                    error_trace!(tx_hash, idx, e);
                    structured_traces.push(StructuredTrace::CALL(CallAction::new(
                        action.from,
                        action.to,
                        action.value,
                        UNKNOWN.to_string(),
                        None,
                        trace_address.clone(),
                    )));
                }
            };

            info!(?tx_hash, trace = ?structured_traces.last());
        }

        Ok(TxTrace { trace: structured_traces, tx_hash: trace.transaction_hash, tx_index })
    }
}
