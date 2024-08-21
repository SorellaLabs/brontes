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
pub struct SubmitBlockRequestExecutionPayloadOneOfAllOf1 {
    #[serde(rename = "transactions", skip_serializing_if = "Option::is_none")]
    pub transactions: Option<Vec<String>>
}

impl SubmitBlockRequestExecutionPayloadOneOfAllOf1 {
    pub fn new() -> SubmitBlockRequestExecutionPayloadOneOfAllOf1 {
        SubmitBlockRequestExecutionPayloadOneOfAllOf1 { transactions: None }
    }
}
