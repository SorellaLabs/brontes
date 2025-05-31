use alloy_primitives::{keccak256, Address, U256};
use alloy_sol_types::{sol_data, SolType};
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    UniswapV2Discovery,
    crate::UniswapV2Factory::createPairCall,
    0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9,
    |deployed_address: Address, trace_index: u64, call_data: createPairCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;
        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::UniswapV2,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    UniswapV3Discovery,
    crate::UniswapV3Factory::createPoolCall,
    0x1F98431c8aD98523631AE4a59f267346ea31F984,
    |deployed_address: Address, trace_index: u64, call_data: createPoolCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;

        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::UniswapV3,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    UniswapV4Discovery,
    crate::UniswapV4Factory::initializeCall,
    0x360E68faCcca8cA495c1B759Fd9EEe466db9FB32,
    |_deployed_address: Address, trace_index: u64, call_data: initializeCall, _| async move {
        let mut token_a = call_data.key.currency0;
        let mut token_b = call_data.key.currency1;

        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        let pool_key = call_data.key;
        type PoolKey = (
            sol_data::Address,
            sol_data::Address,
            sol_data::Uint<256>,
            sol_data::Uint<256>,
            sol_data::Address,
        );

        let encoded_data = PoolKey::abi_encode(&(
            pool_key.currency0,
            pool_key.currency1,
            U256::from(pool_key.fee),
            U256::from(pool_key.tickSpacing as u64),
            pool_key.hooks,
        ));
        let target_address = Address::from_slice(&keccak256(&encoded_data)[..20]);

        vec![NormalizedNewPool {
            pool_address: target_address,
            trace_index,
            protocol: Protocol::UniswapV4,
            tokens: vec![token_a, token_b],
        }]
    }
);

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_types::{normalized_actions::pool::NormalizedNewPool, Protocol};

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_uniswap_v2_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("16bba367585045f6c87ec2beca8243575d7a5891f58c1af5e70bc45de4d3e347"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::UniswapV2,
            pool_address: Address::new(hex!("082366f442ea46a608f3c2c5e7abd5f53a86125b")),
            tokens:       vec![
                hex!("52c6889677E514BDD0f09E32003C15B33E88DccE").into(),
                hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("082366f442ea46a608f3c2c5e7abd5f53a86125b")),
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

    #[brontes_macros::test]
    async fn test_uniswap_v3_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("06c8ae6cc8705d3c6c8da07f2cb14af08ce981788ef237dcd204992ad207ddf1"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::UniswapV3,
            pool_address: Address::new(hex!("602c70f43c7436975aec3113b316e7912d5ee2e3")),
            tokens:       vec![
                hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
                hex!("edB357b55BC2DA1882B629EaDD3DF06202092d69").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("602c70f43c7436975aec3113b316e7912d5ee2e3")),
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
