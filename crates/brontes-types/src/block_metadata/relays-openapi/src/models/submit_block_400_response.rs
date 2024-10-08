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
pub struct SubmitBlock400Response {
    /// Either specific error code in case of invalid request or http status
    /// code
    #[serde(rename = "code", skip_serializing_if = "Option::is_none")]
    pub code:        Option<f32>,
    /// Message describing error
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub message:     Option<String>,
    /// Optional stacktraces, sent when node is in debug mode
    #[serde(rename = "stacktraces", skip_serializing_if = "Option::is_none")]
    pub stacktraces: Option<Vec<String>>,
}

impl SubmitBlock400Response {
    pub fn new() -> SubmitBlock400Response {
        SubmitBlock400Response { code: None, message: None, stacktraces: None }
    }
}
