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
    result:  TxTrace,
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
        let params = json!([format!("0x{}", hex::encode(block_hash)), trace_options]);
        let result: Result<Vec<TraceResult>, RpcError> =
            self.call("debug_traceBlockByHash", params).await;
        result.map(|traces| traces.into_iter().map(|trace| trace.result).collect())
    }

    pub async fn debug_trace_block_by_number(
        &self,
        block_number: u64,
        trace_options: TraceOptions,
    ) -> Result<Vec<TxTrace>, RpcError> {
        let params = json!([format!("0x{:x}", block_number), trace_options]);
        // First try to parse as a single TraceResult
        let result: Result<Vec<TraceResult>, RpcError> =
            self.call("debug_traceBlockByNumber", params).await;
        result.map(|traces| traces.into_iter().map(|trace| trace.result).collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_trace_response_parsing() {
        // The sample JSON response
        let json_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "txHash": "0xac11f10e2b9a822a5a986f1ddb0bf4618b99c40855870ff22f90776aa0682007",
                    "result": {
                        "gas_used": "0x0",
                        "effective_price": "0x0",
                        "block_number": 327340070,
                        "trace": [],
                        "tx_hash": "0xac11f10e2b9a822a5a986f1ddb0bf4618b99c40855870ff22f90776aa0682007",
                        "tx_index": 0,
                        "is_success": true
                    }
                },
                {
                    "txHash": "0xdbc9477ae5709d01af509d709a2a1413ed72ee5aeb83903bc33de8d09c39a09a",
                    "result": {
                        "gas_used": "0x925d5",
                        "effective_price": "0x0",
                        "block_number": 327340070,
                        "trace": [
                            {
                                "trace": {
                                    "type": "call",
                                    "action": {
                                        "callType": "call",
                                        "from": "0xeb1a8834cf6ca6d721e5cb1a8ad472bbf62eef8e",
                                        "gas": "0x1e84800",
                                        "input": "0xc9807539000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000002600000000000000000000000000000000000000000000000000000000000000300000001010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001c0000000000000000000000074d0c0518667458577264b2aac15152b0007af060502050103070604080009000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000006b837000000000000000000000000000000000000000000000000000000000006b8c2d00000000000000000000000000000000000000000000000000000000006b8dd400000000000000000000000000000000000000000000000000000000006b8dd400000000000000000000000000000000000000000000000000000000006b8dd400000000000000000000000000000000000000000000000000000000006b8eca00000000000000000000000000000000000000000000000000000000006b918800000000000000000000000000000000000000000000000000000000006b91f600000000000000000000000000000000000000000000000000000000006bd28800000000000000000000000000000000000000000000000000000000006bd28800000000000000000000000000000000000000000000000000000000000000044d386f5b8f827dfce20681a96333dac17ddf3c8f9c10dbff95d77166d3bd74fdfaaa47e8c96aea3e67c4c2a3333009fa1ddc0338d624e7f1b6d14be4f044e58e2a9f43ee243b6da002e3eb70c9da6ce8895757f5649b7a751dd3a8cb11354634b085d04dc2fd504f10cf35a4ef131675ea8f424bac7e77276526bddf2c619c440000000000000000000000000000000000000000000000000000000000000004482f9b59a985a095b8e445a8f84a8799ebab63a0e1ad7037c7504f107f25541f0e57f8787e68a6c804c183efe6d1dbcc29df551c3bcb3a1f15b2d98d8ecdc81804c9171ee8cdac82bfce4b092f42107ce6fd4f849af257316ad9c75facf481967dec3be8b5b1f11d2eb4fb8d7dbdb61de520ba4ce4b431e0e7f2c9f7ccede0f1",
                                        "to": "0x5ab0b1e2604d4b708721bc3cd1ce962958b4297e",
                                        "value": "0x0"
                                    },
                                    "error": "",
                                    "result": {
                                        "gasUsed": 124036,
                                        "output": "0x"
                                    },
                                    "subtraces": 0,
                                    "traceAddress": []
                                },
                                "logs": [],
                                "msg_sender": "0xeb1a8834cf6ca6d721e5cb1a8ad472bbf62eef8e",
                                "trace_idx": 0
                            }
                        ],
                        "tx_hash": "0xdbc9477ae5709d01af509d709a2a1413ed72ee5aeb83903bc33de8d09c39a09a",
                        "tx_index": 1,
                        "is_success": true
                    }
                }
            ]
        });

        // Create a JsonRpcResponse from the JSON data
        let json_string = json_response.to_string();
        let rpc_response: JsonRpcResponse = serde_json::from_str(&json_string).unwrap();

        // Verify the response fields
        assert_eq!(rpc_response.jsonrpc, "2.0");
        assert_eq!(rpc_response.id, 1);
        assert!(rpc_response.error.is_none());
        assert!(rpc_response.result.is_some());

        // Extract and parse the trace results
        let trace_results: Vec<TraceResult> =
            serde_json::from_value(rpc_response.result.unwrap()).unwrap();

        // Verify the trace results
        assert_eq!(trace_results.len(), 2);

        // First trace
        let first_trace = &trace_results[0];
        assert_eq!(
            first_trace.tx_hash.to_string().to_lowercase(),
            "0xac11f10e2b9a822a5a986f1ddb0bf4618b99c40855870ff22f90776aa0682007".to_lowercase()
        );
        assert_eq!(first_trace.result.tx_index, 0);
        assert_eq!(first_trace.result.block_number, 327340070);
        assert!(first_trace.result.is_success);
        assert!(first_trace.result.trace.is_empty());

        // Second trace
        let second_trace = &trace_results[1];
        assert_eq!(
            second_trace.tx_hash.to_string().to_lowercase(),
            "0xdbc9477ae5709d01af509d709a2a1413ed72ee5aeb83903bc33de8d09c39a09a".to_lowercase()
        );
        assert_eq!(second_trace.result.tx_index, 1);
        assert_eq!(second_trace.result.block_number, 327340070);
        assert!(second_trace.result.is_success);
        assert_eq!(second_trace.result.trace.len(), 1);

        // Verify gas_used and effective_price values
        // "0x925d5" in hex = 599509 in decimal
        assert_eq!(second_trace.result.gas_used, 599509);
        // "0x0" in hex = 0 in decimal
        assert_eq!(second_trace.result.effective_price, 0);

        // Verify a trace element
        let trace_element = &second_trace.result.trace[0];
        assert_eq!(trace_element.trace_idx, 0);

        // Compare addresses with case insensitivity
        let actual_sender = trace_element.msg_sender.to_string().to_lowercase();
        let expected_sender = "0xeb1a8834cf6ca6d721e5cb1a8ad472bbf62eef8e".to_lowercase();
        assert_eq!(actual_sender, expected_sender);

        assert!(trace_element.logs.is_empty());
    }
}
