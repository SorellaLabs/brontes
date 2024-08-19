use alloy_primitives::Address;
use colored::Colorize;
use tracing::info;

use crate::ParserMetricEvents;

/// metric event for traces
#[derive(Clone, Debug)]
pub enum DynamicContractMetricEvent {
    /// recorded a new contract metric
    ContractMetricRecieved(ContractMetric),
}

impl DynamicContractMetricEvent {
    pub(crate) fn get_addr(&self) -> String {
        match self {
            DynamicContractMetricEvent::ContractMetricRecieved(val) => {
                format!("{:#x}", val.address)
            }
        }
    }
}

impl From<DynamicContractMetricEvent> for ParserMetricEvents {
    fn from(val: DynamicContractMetricEvent) -> Self {
        ParserMetricEvents::DynamicContractMetricRecieved(val)
    }
}

#[derive(Clone, Debug)]
pub struct ContractMetric {
    pub address:         Address,
    pub function_called: String,
}

impl ContractMetric {
    pub fn new(address: Address, function_called: String) -> Self {
        Self { address, function_called }
    }

    pub fn trace(&self) {
        let message = format!(
            "Successfuly Parsed Contract: {} --- Function Called: {}",
            format!("{:#x}", self.address).bright_blue().bold(),
            self.function_called.to_string().bright_blue().bold(),
        );
        info!(message = message);
    }
}
