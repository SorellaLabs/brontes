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
            protocol: Protocol::BalancerV1CRP,
            pool_address: deployed_address,
            tokens: vec![],
        }]
    }
);



// Balancer V2
discovery_impl!(
    BalancerV2ComposableStablePoolV5Discovery,
    crate::BalancerV2ComposableStablePoolFactoryV5::createCall,
    0xDB8d758BCb971e482B2C45f7F8a7740283A1bd3A,
    |deployed_address: Address, trace_index: u64, call_data: createCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::BalancerV2,
            pool_address: deployed_address,
            tokens: call_data.tokens,
        }]
    }
);

// Deprecated but had pool creations.
discovery_impl!(
    BalancerV2ComposableStablePoolV4Discovery,
    crate::BalancerV2ComposableStablePoolFactoryV4::createCall,
    0xfADa0f4547AB2de89D1304A668C39B3E09Aa7c76,
    |deployed_address: Address, trace_index: u64, call_data: createCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::BalancerV2,
            pool_address: deployed_address,
            tokens: call_data.tokens,
        }]
    }
);

// Deprecated but had pool creations.
discovery_impl!(
    BalancerV2ComposableStablePoolV3Discovery,
    crate::BalancerV2ComposableStablePoolFactoryV4::createCall,
    0xdba127fBc23fb20F5929C546af220A991b5C6e01,
    |deployed_address: Address, trace_index: u64, call_data: createCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::BalancerV2,
            pool_address: deployed_address,
            tokens: call_data.tokens,
        }]
    }
);


// Smart Pool Factory
//  fub4

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_types::{normalized_actions::pool::NormalizedNewPool, Protocol};

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_balancer_v1_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("f5b9b2c23fa3ddf58c31a9377d37439740913f526910cca947c0a3e4bb9bb1d7"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::BalancerV1,
            pool_address: Address::new(hex!("1FA0d58e663017cdd80B87fd24C46818364fc9B6")),
            tokens:       vec![],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("1FA0d58e663017cdd80B87fd24C46818364fc9B6")),
                |mut pool| {
                    assert_eq!(pool.len(), 1);
                    let pool = pool.remove(0);
                    assert_eq!(pool.protocol, eq_create.protocol);
                    assert_eq!(pool.pool_address, eq_create.pool_address);
                    assert_eq!(pool.tokens, eq_create.tokens);
                },
            )
            .await
            .unwrap();
    }
}
