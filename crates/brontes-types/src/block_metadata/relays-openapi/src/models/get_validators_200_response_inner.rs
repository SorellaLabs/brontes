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
pub struct GetValidators200ResponseInner {
    #[serde(rename = "slot", skip_serializing_if = "Option::is_none")]
    pub slot:            Option<String>,
    #[serde(rename = "validator_index", skip_serializing_if = "Option::is_none")]
    pub validator_index: Option<String>,
    #[serde(rename = "entry", skip_serializing_if = "Option::is_none")]
    pub entry:           Option<Box<crate::models::GetValidators200ResponseInnerEntry>>,
}

impl GetValidators200ResponseInner {
    pub fn new() -> GetValidators200ResponseInner {
        GetValidators200ResponseInner {
            slot:            None,
            validator_index: None,
            entry:           None,
        }
    }
}