use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedNewPool, structured_trace::CallInfo};

/// governor -(execute)-> comet factory -(deploy)-> comet (based on comet
/// so based on the clone calldata we can extract the tokens
/// configurator)
action_impl!(
    Protocol::CompoundV3,
    crate::CometConfigurator::setConfigurationCall,
    NewPool,
    [],
    call_data: true,
    |info: CallInfo, call_data:setConfigurationCall, _| {
        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::CompoundV3,
            pool_address: call_data.cometProxy,
            tokens: vec![call_data.newConfiguration.baseToken]
        })
    }
);
action_impl!(
    Protocol::CompoundV2,
    crate::CErc20Delegate::initialize_0Call,
    NewPool,
    [],
    call_data: true,
    |info: CallInfo, _call_data: initialize_0Call, _| {
        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::CompoundV2,
            pool_address: info.from_address,
            tokens: vec![info.from_address]
        })
    }
);

action_impl!(
    Protocol::CompoundV2,
    crate::CErc20Delegate::initialize_1Call,
    NewPool,
    [],
    call_data: true,
    |info: CallInfo, _call_data: initialize_1Call, _| {
        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::CompoundV2,
            pool_address: info.from_address,
            tokens: vec![info.from_address]
        })

    }
);

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, B256};
    use brontes_types::{
        normalized_actions::{pool::NormalizedNewPool, Action},
        Protocol, TreeSearchBuilder,
    };

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_compound_v2_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let compound_v2_discovery =
            B256::from(hex!("090ce7d33359e5d288ce169f41bb3d2cb55ac17b026a10cf80b3fc4f0c85c827"));

        let eq_action = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CompoundV2,
            pool_address: hex!("5d3a536e4d6dbd6114cc1ead35777bab948e3643").into(),
            tokens:       vec![hex!("5d3a536e4d6dbd6114cc1ead35777bab948e3643").into()],
        });
        let search = TreeSearchBuilder::default().with_action(Action::is_new_pool);

        classifier_utils
            .contains_action(compound_v2_discovery, 0, eq_action, search)
            .await
            .unwrap();
    }
}
