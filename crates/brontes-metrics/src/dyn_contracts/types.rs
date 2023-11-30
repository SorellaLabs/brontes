use colored::Colorize;
use reth_primitives::Address;
use tracing::info;

use crate::PoirotMetricEvents;

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

impl From<DynamicContractMetricEvent> for PoirotMetricEvents {
    fn from(val: DynamicContractMetricEvent) -> Self {
        PoirotMetricEvents::DynamicContractMetricRecieved(val)
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
