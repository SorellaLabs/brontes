/*
 * Relay API
 *
 * API specification for MEV-Boost PBS relays.
 *
 * The version of the OpenAPI document: dev
 *
 * Generated by: https://openapi-generator.tech
 */

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SubmitBlockRequestExecutionPayloadOneOf1AllOf1 {
    #[serde(rename = "transactions", skip_serializing_if = "Option::is_none")]
    pub transactions: Option<Vec<String>>,
    #[serde(rename = "withdrawals", skip_serializing_if = "Option::is_none")]
    pub withdrawals:  Option<Vec<crate::models::SubmitBlockRequestExecutionPayloadOneOf1AllOf1WithdrawalsInner>>
}

impl SubmitBlockRequestExecutionPayloadOneOf1AllOf1 {
    pub fn new() -> SubmitBlockRequestExecutionPayloadOneOf1AllOf1 {
        SubmitBlockRequestExecutionPayloadOneOf1AllOf1 { transactions: None, withdrawals: None }
    }
}
