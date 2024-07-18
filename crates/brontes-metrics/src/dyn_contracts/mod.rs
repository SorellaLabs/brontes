#![allow(dead_code)]
use std::collections::HashMap;

use metrics::Counter;
use reth_metrics::Metrics;
use tracing::trace;

use self::types::DynamicContractMetricEvent;
pub mod types;

#[derive(Debug, Default, Clone)]
pub struct DynamicContractMetrics {
    contracts: ContractMetrics,
    functions: HashMap<String, ContractFunctionMetrics>,
}

impl DynamicContractMetrics {
    /// Returns existing or initializes a new instance of [ContractMetrics]
    pub(crate) fn get_contract_metrics(&mut self, _address: String) -> &mut ContractMetrics {
        &mut self.contracts
    }

    /// Returns existing or initializes a new instance of
    /// [ContractFunctionMetrics]
    pub(crate) fn get_function_metrics(
        &mut self,
        function_name: String,
    ) -> &mut ContractFunctionMetrics {
        self.functions
            .entry(function_name.clone())
            .or_insert_with(|| {
                ContractFunctionMetrics::new_with_labels(&[("functions", function_name)])
            })
    }

    pub(crate) fn handle_event(&mut self, event: DynamicContractMetricEvent) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            DynamicContractMetricEvent::ContractMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
        }
    }
}

#[metrics(scope = "contracts")]
#[derive(Metrics, Clone)]
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
