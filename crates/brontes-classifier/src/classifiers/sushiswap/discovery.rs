use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    SushiSwapV2Discovery,
    crate::UniswapV2Factory::createPairCall,
    0xc35DADB65012eC5796536bD9864eD8773aBc74C4,
    |deployed_address: Address, trace_index: u64, call_data: createPairCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;
        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::SushiSwapV2,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    SushiSwapV3Discovery,
    crate::UniswapV3Factory::createPoolCall,
    0x1af415a1EbA07a4986a52B6f2e7dE7003D82231e,
    |deployed_address: Address, trace_index: u64, call_data: createPoolCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;

        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::SushiSwapV3,
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
    async fn test_sushiswap_v2_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("d0acb944bf0f45dddc92e73376825a6395a3badf82f86283fa0b3ac5139a46eb"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::SushiSwapV2,
            pool_address: Address::new(hex!("4c5be0fea74c33455f81c85561146bdaf09633da")),
            tokens:       vec![
                hex!("189564397643D9e6173A002f1BA98da7d40a0FA6").into(),
                hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("4c5be0fea74c33455f81c85561146bdaf09633da")),
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
