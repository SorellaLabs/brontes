use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

// Balancer V1 pool factory. See balancer pool lifecycle:
// https://balancer.gitbook.io/balancer/core-concepts/protocol/pool-lifecycle
discovery_impl!(
    BalancerV1CoreDiscovery,
    crate::BalancerV1CorePoolFactory::newBPoolCall,
    0x9424B1412450D0f8Fc2255FAf6046b98213B76Bd,
    |deployed_address: Address, trace_index: u64, _call_data: newBPoolCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::BalancerV1,
            pool_address: deployed_address,
            tokens: vec![],
        }]
    }
);

discovery_impl!(
    BalancerV1SmartPoolDiscovery,
    crate::BalancerV1SmartPoolFactory::newCrpCall,
    0xed52D8E202401645eDAD1c0AA21e872498ce47D0,
    |deployed_address: Address, trace_index: u64, _call_data: newCrpCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::BalancerV1,
            pool_address: deployed_address,
            tokens: vec![],
        }]
    }
);

// Smart Pool Factory
//  fub4

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_types::{
        normalized_actions::{pool::NormalizedNewPool, Actions},
        Protocol, TreeSearchBuilder,
    };

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_balancer_v1_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let balancer_v1_discovery_hash =
            B256::from(hex!("f5b9b2c23fa3ddf58c31a9377d37439740913f526910cca947c0a3e4bb9bb1d7"));

        let eq_action = Actions::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::BalancerV1,
            pool_address: Address::from(hex!("1FA0d58e663017cdd80B87fd24C46818364fc9B6")),
            tokens:       vec![],
        });
        let search = TreeSearchBuilder::default().with_action(Actions::is_new_pool);

        classifier_utils
            .contains_action(balancer_v1_discovery_hash, 0, eq_action, search)
            .await
            .unwrap();
    }
}
