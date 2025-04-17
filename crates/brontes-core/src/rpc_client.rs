//! A temporary custom RPC client implementation for Brontes tracer.
//!
//! This module provides a custom RPC client implementation specifically for the
//! Brontes tracer, as the functionality needed (particularly
//! debug_traceBlockByHash and debug_traceBlockByNumber) is not currently
//! supported by the alloy provider.
//!
//! The client handles JSON-RPC communication with Ethereum nodes, specifically
//! focusing on transaction tracing functionality. It provides methods for
//! tracing blocks by hash or number, and includes comprehensive error handling
//! and logging for debugging purposes.
//!
//! Note: This is a temporary solution until the alloy provider adds support for
//! these tracing methods.

use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

use brontes_types::structured_trace::TxTrace;
use reqwest::{Client, Error as ReqwestError};
use reth_primitives::{hex, B256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug)]
pub enum RpcError {
    RequestError(ReqwestError),
    JsonError(serde_json::Error),
    RpcError { code: i64, message: String },
    UnexpectedResponse(String),
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcError::RequestError(e) => write!(f, "Request error: {}", e),
            RpcError::JsonError(e) => write!(f, "JSON error: {}", e),
            RpcError::RpcError { code, message } => write!(f, "RPC error {}: {}", code, message),
            RpcError::UnexpectedResponse(s) => write!(f, "Unexpected response: {}", s),
        }
    }
}

impl From<ReqwestError> for RpcError {
    fn from(err: ReqwestError) -> Self {
        RpcError::RequestError(err)
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(err: serde_json::Error) -> Self {
        RpcError::JsonError(err)
    }
}

impl std::error::Error for RpcError {}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method:  String,
    params:  Value,
    id:      u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    result:  Option<Value>,
    error:   Option<JsonRpcError>,
    id:      u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceResult {
    tx_hash: B256,
    result:  Vec<TxTrace>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code:    i64,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceOptions {
    pub tracer: String,
}

#[derive(Debug)]
pub struct RpcClient {
    endpoint: String,
    client:   Client,
    id:       AtomicU64,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            client:   self.client.clone(),
            id:       AtomicU64::new(self.id.load(Ordering::SeqCst)),
        }
    }
}

impl RpcClient {
    pub fn new(url: reqwest::Url) -> Self {
        let endpoint = url.to_string();
        Self { endpoint, client: Client::new(), id: AtomicU64::new(1) }
    }

    async fn call<T: for<'a> Deserialize<'a>>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, RpcError> {
        tracing::info!(target: "rpc_client", "calling method: {:?}", method);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: self.id.load(Ordering::SeqCst),
        };
        tracing::info!(target: "rpc_client", "request: {:?}", request);
        self.id.fetch_add(1, Ordering::SeqCst);

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let json: JsonRpcResponse = response.json().await?;
        if let Some(error) = json.error {
            return Err(RpcError::RpcError { code: error.code, message: error.message });
        }

        if let Some(result) = json.result {
            match serde_json::from_value::<T>(result) {
                Ok(parsed_result) => Ok(parsed_result),
                Err(err) => Err(RpcError::JsonError(err)),
            }
        } else {
            Err(RpcError::UnexpectedResponse("No result in JSON-RPC response".to_string()))
        }
    }

    pub async fn debug_trace_block_by_hash(
        &self,
        block_hash: B256,
        trace_options: TraceOptions,
    ) -> Result<Vec<TxTrace>, RpcError> {
        tracing::info!(target: "rpc_client", "debug_trace_block_by_hash: {:?}", block_hash);
        let params = json!([format!("0x{}", hex::encode(block_hash)), trace_options]);
        let result: Result<TraceResult, RpcError> =
            self.call("debug_traceBlockByHash", params).await;
        tracing::info!(target: "rpc_client", "debug_trace_block_by_hash result: {:?}", result);
        result.map(|traces| traces.result)
    }

    pub async fn debug_trace_block_by_number(
        &self,
        block_number: u64,
        trace_options: TraceOptions,
    ) -> Result<Vec<TxTrace>, RpcError> {
        tracing::info!(target: "rpc_client", "debug_trace_block_by_number: {:?}", block_number);
        let params = json!([format!("0x{:x}", block_number), trace_options]);

        // First try to parse as a single TraceResult
        let result: Result<TraceResult, RpcError> =
            self.call("debug_traceBlockByNumber", params).await;
        tracing::info!(target: "rpc_client", "debug_trace_block_by_number result: {:?}", result);
        result.map(|traces| traces.result)
    }
}
