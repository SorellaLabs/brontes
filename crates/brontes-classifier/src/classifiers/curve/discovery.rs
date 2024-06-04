use std::sync::Arc;

use alloy_primitives::{Address, U256};
use brontes_macros::{curve_discovery_impl, discovery_impl};
use brontes_pricing::make_call_request;
use brontes_types::{
    normalized_actions::pool::NormalizedNewPool, traits::TracingProvider, Protocol,
};

curve_discovery_impl!(
    CurveV1,
    crate::CurveV1MetapoolFactory,
    0x0959158b6040d32d04c301a72cbfd6b39e21c9ae,
    (1, 0)
);

curve_discovery_impl!(
    CurveV2,
    crate::CurveV2MetapoolFactory,
    0xb9fc157394af804a3578134a6585c0dc9cc990d4,
    (2, 3)
);

curve_discovery_impl!(
    CurvecrvUSD,
    crate::CurvecrvUSDFactory,
    0x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d,
    (2, 3)
);

discovery_impl!(
    CurveCryptoSwapDiscovery,
    crate::CurveCryptoSwapFactory::deploy_poolCall,
    0xf18056bbd320e96a48e3fbf8bc061322531aac99,
    |deployed_address: Address, trace_index: u64, call_data: deploy_poolCall, _| async move {
        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::CurveCryptoSwapPool,
            pool_address: deployed_address,
            tokens: call_data._coins.to_vec(),
        }]
    }
);

discovery_impl!(
    CurveTriCryptoDiscovery,
    crate::CurveTriCryptoFactory::deploy_poolCall,
    0x0c0e5f2ff0ff18a3be9b835635039256dc4b4963,
    |deployed_address: Address, trace_index: u64, call_data: deploy_poolCall, _| async move {
        let mut tokens = call_data._coins.to_vec();

        if !tokens.contains(&call_data._weth) {
            tokens.push(call_data._weth);
        }

        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::CurveCryptoSwapPool,
            pool_address: deployed_address,
            tokens: call_data._coins.to_vec(),
        }]
    }
);

alloy_sol_types::sol!(
    function coins(int128 arg0) external view returns (address);
    function coins(uint256 arg0) external view returns (address);
);

async fn query_base_pool<T: TracingProvider>(
    tracer: &Arc<T>,
    base_pool: &Address,
    is_meta: bool,
) -> Vec<Address> {
    let mut result = Vec::new();
    let mut i = 0;
    loop {
        let addr = if is_meta {
            let Ok(call) = make_call_request(
                coins_1Call { arg0: U256::from(i as u64) },
                tracer,
                *base_pool,
                None,
            )
            .await
            else {
                break;
            };
            call._0
        } else {
            let Ok(call) =
                make_call_request(coins_0Call { arg0: i }, tracer, *base_pool, None).await
            else {
                break;
            };
            call._0
        };

        i += 1;
        result.push(addr);
    }
    result
}

async fn parse_plain_pool<const N: usize>(
    protocol: Protocol,
    deployed_address: Address,
    trace_index: u64,
    tokens: [Address; N],
) -> Vec<NormalizedNewPool> {
    let tokens = tokens.into_iter().filter(|t| t != &Address::ZERO).collect();

    vec![NormalizedNewPool { pool_address: deployed_address, trace_index, protocol, tokens }]
}

async fn parse_meta_pool<T: TracingProvider>(
    protocol: Protocol,
    deployed_address: Address,
    base_pool: Address,
    meta_token: Address,
    trace_index: u64,
    tracer: Arc<T>,
) -> Vec<NormalizedNewPool> {
    let mut tokens = query_base_pool(&tracer, &base_pool, true).await;
    tokens.push(meta_token);

    vec![NormalizedNewPool { pool_address: deployed_address, trace_index, protocol, tokens }]
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, B256};
    use brontes_types::{
        normalized_actions::{pool::NormalizedNewPool, Action},
        Protocol, TreeSearchBuilder,
    };
    use brontes_classifier::test_utils::ClassifierTestUtils;

    // #[brontes_macros::test]
    // async fn test_curve_v1_discovery() {
    //     let classifier_utils = ClassifierTestUtils::new().await;
    //     let curve_v1_discovery =
    //         B256::from(hex!("4efad6165d8c61926f02a15412653f2bafe499c1cbc505176b9f14041fd8ca5c"));

    //     let eq_action = Action::NewPool(NormalizedNewPool {
    //         trace_index:  1,
    //         protocol:     Protocol::CompoundV2,
    //         pool_address: hex!("bEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7").into(),
    //         tokens:       vec![hex!("a839D74FE0E456c8fCd4d9bb8759861aFcBbD1CE").into()],
    //     });
    //     let search = TreeSearchBuilder::default().with_action(Action::is_new_pool);

    //     classifier_utils
    //         .contains_action(curve_v1_discovery, 0, eq_action, search)
    //         .await
    //         .unwrap();
    // }

    #[brontes_macros::test]
    async fn test_curve_v2_discovery() {
    }

    #[brontes_macros::test]
    async fn test_curve_crvUSD_discovery() {
    }

    #[brontes_macros::test]
    async fn test_curve_crypto_swap_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let curve_crypto_swap_discovery =
            B256::from(hex!("b8225567ede93bc296b5ac263d5419f8910bc6c93554fbf5d7a643a945011743"));

        let eq_action = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CurveCryptoSwapPool,
            pool_address: hex!("F18056Bbd320E96A48e3Fbf8bC061322531aac99").into(),
            tokens:       vec![hex!("d4092e32b3517ee0daa4da48fdc40cab6058e187").into()],
        });
        let search = TreeSearchBuilder::default().with_action(Action::is_new_pool);
        println!("{search}");
        classifier_utils
            .contains_action(curve_crypto_swap_discovery, 0, eq_action, search)
            .await
            .unwrap();
    }
}
