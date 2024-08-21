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
pub struct SubmitBlockRequestExecutionPayload {
    #[serde(rename = "parent_hash", skip_serializing_if = "Option::is_none")]
    pub parent_hash:      Option<String>,
    /// An address on the execution (Ethereum 1) network.
    #[serde(rename = "fee_recipient", skip_serializing_if = "Option::is_none")]
    pub fee_recipient:    Option<String>,
    #[serde(rename = "state_root", skip_serializing_if = "Option::is_none")]
    pub state_root:       Option<String>,
    #[serde(rename = "receipts_root", skip_serializing_if = "Option::is_none")]
    pub receipts_root:    Option<String>,
    #[serde(rename = "logs_bloom", skip_serializing_if = "Option::is_none")]
    pub logs_bloom:       Option<String>,
    #[serde(rename = "prev_randao", skip_serializing_if = "Option::is_none")]
    pub prev_randao:      Option<String>,
    #[serde(rename = "block_number", skip_serializing_if = "Option::is_none")]
    pub block_number:     Option<String>,
    #[serde(rename = "gas_limit", skip_serializing_if = "Option::is_none")]
    pub gas_limit:        Option<String>,
    #[serde(rename = "gas_used", skip_serializing_if = "Option::is_none")]
    pub gas_used:         Option<String>,
    #[serde(rename = "timestamp", skip_serializing_if = "Option::is_none")]
    pub timestamp:        Option<String>,
    /// Extra data on the execution (Ethereum 1) network.
    #[serde(rename = "extra_data", skip_serializing_if = "Option::is_none")]
    pub extra_data:       Option<String>,
    #[serde(rename = "base_fee_per_gas", skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<String>,
    #[serde(rename = "block_hash", skip_serializing_if = "Option::is_none")]
    pub block_hash:       Option<String>,
    #[serde(rename = "transactions", skip_serializing_if = "Option::is_none")]
    pub transactions:     Option<Vec<String>>,
    #[serde(rename = "withdrawals", skip_serializing_if = "Option::is_none")]
    pub withdrawals:      Option<Vec<crate::models::SubmitBlockRequestExecutionPayloadOneOf1AllOf1WithdrawalsInner>>
}

impl SubmitBlockRequestExecutionPayload {
    pub fn new() -> SubmitBlockRequestExecutionPayload {
        SubmitBlockRequestExecutionPayload {
            parent_hash:      None,
            fee_recipient:    None,
            state_root:       None,
            receipts_root:    None,
            logs_bloom:       None,
            prev_randao:      None,
            block_number:     None,
            gas_limit:        None,
            gas_used:         None,
            timestamp:        None,
            extra_data:       None,
            base_fee_per_gas: None,
            block_hash:       None,
            transactions:     None,
            withdrawals:      None
        }
    }
}
