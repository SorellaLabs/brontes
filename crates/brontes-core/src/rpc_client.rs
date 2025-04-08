use std::fmt;

use reqwest::{Client, Error as ReqwestError};
use reth_primitives::{hex, B256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use brontes_types::structured_trace::TxTrace;
use std::sync::atomic::{AtomicU64, Ordering};


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
            client: self.client.clone(),
            id: AtomicU64::new(self.id.load(Ordering::SeqCst)),
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
            
        // Debug print the raw response text
        let response_text = response.text().await?;
        tracing::debug!(target: "rpc_client", "Raw response: {}", response_text);
        
        // Parse the text back to JSON
        let response: JsonRpcResponse = serde_json::from_str(&response_text)?;

        if let Some(error) = response.error {
            return Err(RpcError::RpcError { code: error.code, message: error.message });
        }

        if let Some(result) = response.result {
            Ok(serde_json::from_value(result)?)
        } else {
            Err(RpcError::UnexpectedResponse("No result or error in response".to_string()))
        }
    }

    pub async fn debug_trace_block_by_hash(
        &self,
        block_hash: B256,
        trace_options: TraceOptions,
    ) -> Result<TxTrace, RpcError> {
        tracing::info!(target: "rpc_client", "debug_trace_block_by_hash: {:?}", block_hash);
        let params = json!([
            format!("0x{}", hex::encode(block_hash.0)),
            trace_options
        ]);
        let result = self.call("debug_traceBlockByHash", params).await;
        tracing::info!(target: "rpc_client", "debug_trace_block_by_hash result: {:?}", result);
        result
    }

    pub async fn debug_trace_block_by_number(
        &self,
        block_number: u64,
        trace_options: TraceOptions,
    ) -> Result<TxTrace, RpcError> {
        tracing::info!(target: "rpc_client", "debug_trace_block_by_number: {:?}", block_number);
        let params = json!([
            format!("0x{:x}", block_number),
            trace_options
        ]);
        let result = self.call("debug_traceBlockByNumber", params).await; 
        tracing::info!(target: "rpc_client", "debug_trace_block_by_number result: {:?}", result);
        result
    }
}
