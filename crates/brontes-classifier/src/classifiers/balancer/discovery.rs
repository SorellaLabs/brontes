use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::{types::DiscoveredPool, Protocol};

discovery_impl!(
    BalancerV1Decoder,
    crate::BalancerV1Factory::newBPoolCall,
    0x9424B1412450D0f8Fc2255FAf6046b98213B76Bd,
    |deployed_address: Address, _: newBPoolCall, _| async move {
        vec![DiscoveredPool::new(vec![], deployed_address, Protocol::BalancerV1)]
    }
);
