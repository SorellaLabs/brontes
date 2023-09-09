use metrics::Counter;
use reth_metrics::Metrics;
use std::collections::HashMap;
use tracing::trace;

use self::types::DynamicContractMetricEvent;

pub mod types;

#[derive(Debug, Default, Clone)]
pub struct DynamicContractMetrics {
    contracts: HashMap<String, ContractMetrics>,
    function: HashMap<String, ContractMetrics>,
}

impl DynamicContractMetrics {
    /// Returns existing or initializes a new instance of [ContractMetrics]
    pub(crate) fn get_contract_metrics(&mut self, address: String) -> &mut ContractMetrics {
        self.contracts
            .entry(address.clone())
            .or_insert_with(|| ContractMetrics::new_with_labels(&[("contracts", address)]))
    }

    /// Returns existing or initializes a new instance of [ContractFunctionMetrics]
    pub(crate) fn get_function(
        &mut self,
        address: String,
        function_name: String,
    ) -> &mut ContractMetrics {
        let this = format!("{}_{}", address, function_name);
        self.contracts
            .entry(this.clone())
            .or_insert_with(|| ContractMetrics::new_with_labels(&[("functions", this)]))
    }

    pub(crate) fn handle_event(&mut self, event: DynamicContractMetricEvent) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            DynamicContractMetricEvent::ContractMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
        }
    }
}

#[derive(Metrics, Clone)]
#[metrics(scope = "contracts")]
pub(crate) struct ContractMetrics {
    /// The number of times the contract has been called
    pub(crate) times_called: Counter,
}

#[derive(Metrics, Clone)]
#[metrics(scope = "contract_functions")]
pub(crate) struct ContractFunctionMetrics {
    /// The number of times the function on the contract has been called
    pub(crate) times_called: Counter,
}
