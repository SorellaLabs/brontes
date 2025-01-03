use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    CamelotV2Discovery,
    crate::CamelotV2Factory::createPairCall,
    0x6EcCab422D763aC031210895C81787E87B43A652,
    |deployed_address: Address, trace_index: u64, call_data: createPairCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;
        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::CamelotV2,
            tokens: vec![token_a, token_b],
        }]
    }
);

discovery_impl!(
    CamelotV3Discovery,
    crate::CamelotV3Factory::createPoolCall,
    0x1a3c9B1d2F0529D97f2afC5136Cc23e58f1FD35B,
    |deployed_address: Address, trace_index: u64, call_data: createPoolCall, _| async move {
        let mut token_a = call_data.tokenA;
        let mut token_b = call_data.tokenB;

        if token_a > token_b {
            std::mem::swap(&mut token_a, &mut token_b)
        }

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::CamelotV3,
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
    async fn test_camelot_v2_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("d0acb944bf0f45dddc92e73376825a6395a3badf82f86283fa0b3ac5139a46eb"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CamelotV2,
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
