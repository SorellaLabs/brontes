use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    PancakeSwapV3Discovery,
    crate::PancakeSwapV3PoolDeployer::deployCall,
    0x41ff9aa7e16b8b1a8a8dc4f0efacd93d02d071c9,
    |deployed_address: Address, trace_index: u64, call_data: deployCall, _| async move {
        let token_a = call_data.token0;
        let token_b = call_data.token1;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::PancakeSwapV3,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    PancakeSwapV2Discovery,
    crate::UniswapV2Factory::createPairCall,
    0x1097053Fd2ea711dad45caCcc45EfF7548fCB362,
    |deployed_address: Address, trace_index: u64, call_data: createPairCall, _| async move {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::PancakeSwapV2,
            tokens: vec![token_a, token_b],
        }]
    }
);
// Add v2 : 0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73

#[cfg(test)]
pub mod test {
    use alloy_primitives::{hex, Address, TxHash};
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::pool::NormalizedNewPool, Protocol,
    };

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_pancake_v3_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            TxHash::new(hex!("2b16d7a3937375d50b29bbec621b3f33bee00c76d1f4c907ae483fa49f63e2f1"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::PancakeSwapV3,
            pool_address: Address::new(hex!("Ed4D5317823Ff7BC8BB868C1612Bb270a8311179")),
            tokens:       vec![
                Address::new(hex!("186eF81fd8E77EEC8BfFC3039e7eC41D5FC0b457")),
                TokenInfoWithAddress::usdt().address,
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("Ed4D5317823Ff7BC8BB868C1612Bb270a8311179")),
                |mut pool| {
                    assert_eq!(pool.len(), 1);
                    let pool = pool.remove(0);
                    assert_eq!(pool.protocol, eq_create.protocol);
                    assert_eq!(pool.tokens, eq_create.tokens);
                },
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_pancake_v3_discovery_failing() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            TxHash::new(hex!("a3383ac63fd07e0f021e6fbf39a586730464e7eced180542f9687ad896dcd938"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::PancakeSwapV3,
            pool_address: Address::new(hex!("Ed4D5317823Ff7BC8BB868C1612Bb270a8311179")),
            tokens:       vec![
                Address::new(hex!("186eF81fd8E77EEC8BfFC3039e7eC41D5FC0b457")),
                TokenInfoWithAddress::usdt().address,
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("bc7766ae74f38f251683633d50cc2c1cd14af948")),
                |mut pool| {
                    assert_eq!(pool.len(), 1);
                    let pool = pool.remove(0);
                    assert_eq!(pool.protocol, eq_create.protocol);
                    assert_eq!(pool.tokens, eq_create.tokens);
                },
            )
            .await
            .unwrap();
    }
}
