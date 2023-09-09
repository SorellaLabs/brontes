use colored::Colorize;
use reth_primitives::H160;
use tracing::info;

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

#[derive(Clone, Debug)]
pub struct ContractMetric {
    pub address: H160,
    pub function_called: String,
}

impl ContractMetric {
    pub fn new(address: H160, function_called: String) -> Self {
        Self { address, function_called }
    }

    pub fn trace(&self) {
        let message = format!(
            "Successfuly Parsed Contract: {} --- Function Called: {}",
            format!("{:#x}", self.address).bright_blue().bold(),
            format!("{}", self.function_called).bright_blue().bold(),
        );
        info!(message = message);
    }
}
