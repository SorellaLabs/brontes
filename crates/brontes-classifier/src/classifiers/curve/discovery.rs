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
            protocol: Protocol::CurveTriCryptoPool,
            pool_address: deployed_address,
            tokens: call_data._coins.to_vec(),
        }]
    }
);

alloy_sol_types::sol!(
    function coins(int128 arg0) external view returns (address);
    function coins(uint256 arg0) external view returns (address);
);

/// query_base_pool returns a Vec<Address> of the tokens used by base_pool.
/// It attempts to use the `coins` method with i128 and U256 argument, sequentially.
async fn query_base_pool<T: TracingProvider>(
    tracer: &Arc<T>,
    base_pool: &Address,
) -> Vec<Address> {
    let mut result = Vec::new();
    let mut i = 0i128;
    loop {
        match make_call_request(coins_0Call { arg0: i }, tracer, *base_pool, None).await {
            Ok(call_return) => {
                i += 1;
                result.push(call_return._0);
            }
            Err(_) => break,
        }
    }
    if result.len() > 0 { return result; }

    let mut i = U256::from(0);
    loop {
        match make_call_request(coins_1Call { arg0: i }, tracer, *base_pool, None).await {
            Ok(call_return) => {
                i += U256::from(1);
                result.push(call_return._0);
            }
            Err(_) => break,
        }
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
    let mut tokens = query_base_pool(&tracer, &base_pool).await;
    tokens.push(meta_token);

    vec![NormalizedNewPool { pool_address: deployed_address, trace_index, protocol, tokens }]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy_primitives::{hex, Address, B256, FixedBytes};
    use brontes_types::{normalized_actions::pool::NormalizedNewPool, Protocol};
    use reth_rpc_types::trace::parity::TraceResults;

    use crate::test_utils::ClassifierTestUtils;

    use super::query_base_pool;

    async fn verify_discovery(tx: FixedBytes<32>, protocol: Protocol, pool_address: Address, tokens: Vec<Address>) {
        let utils = ClassifierTestUtils::new().await;

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol,
            pool_address,
            tokens,
        };

        utils
            .test_discovery_classification(
                tx,
                pool_address,
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
    async fn test_curve_v1_metapool_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("49878ff3e5e0de4f45c875c94977c154a4f6bea22640f72e85a18434672e3bb2"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CurveV1MetaPool,
            pool_address: Address::new(hex!("5a6a4d54456819380173272a5e8e9b9904bdf41b")),
            tokens:       vec![
                hex!("6b175474e89094c44da98b954eedeac495271d0f").into(),
                hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").into(),
                hex!("dac17f958d2ee523a2206206994597c13d831ec7").into(),
                hex!("99d8a9c45b2eca8864373a26d1459e3dff1e17f3").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("5a6a4d54456819380173272a5e8e9b9904bdf41b")),
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
    async fn test_curve_v2_metapool_plainpool1_discovery() {
        verify_discovery(
            hex!("6f9223d991fa3620d7295f5c7e96581bbbfcd6eb03054ebd85ed3b1d06472217").into(), 
            Protocol::CurveV2PlainPool, 
            hex!("d0e24cb3e766581952dbf258b78e89c63a37f5fb").into(),
            vec![
                hex!("1Ee4dE3CD1505Ddb2e60C20651A4aB7FfABDc8F6").into(),
                hex!("246BE97fda42375c39E21377Ad80D8290AfdB994").into(),
            ]
        ).await;
    }

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_plainpool2_discovery() {
        verify_discovery(
            hex!("cf98501f3158251d2659c556f74e3429fbee4671d8b443269707550481f8d915").into(), 
            Protocol::CurveV2PlainPool, 
            hex!("0ad66fec8db84f8a3365ada04ab23ce607ac6e24").into(),
            vec![
                hex!("11EBe21e9d7BF541A18e1E3aC94939018Ce88F0b").into(),
                hex!("3432B6A60D23Ca0dFCa7761B7ab56459D9C964D0").into(),
            ]
        ).await;
    }

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_plainpool3_discovery() {
        verify_discovery(
            hex!("6d80735b4a78471669dd66301df030be9e71447d7c35a40331a3f55a8b74ec4e").into(), 
            Protocol::CurveV2PlainPool, 
            hex!("5ec58c7def28e0c2470cb8bd7ab9c4ebed0a86b7").into(),
            vec![
                hex!("57Ab1ec28D129707052df4dF418D58a2D46d5f51").into(),
                hex!("b2F30A7C980f052f02563fb518dcc39e6bf38175").into(),
                hex!("43833f0C2025dFA80b5556574FAA11CBf7F3f4eB").into(),
            ]
        ).await;
    }

    #[brontes_macros::test]
    async fn test_query_base_pool() {
        let utils = ClassifierTestUtils::new().await;
        let tracer = utils.get_tracing_provider();

        let base_pool = Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714"));
        let is_meta = false;
        let actual_tokens = query_base_pool(&tracer, &base_pool).await;
        assert_eq!(actual_tokens, vec![
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"))]);
    }

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_metapool1_discovery() {
        verify_discovery(
            hex!("11dfcfa281837030ac8c994828fe174fdd75cfa8a66971b4b84fb38a1bb08597").into(), 
            Protocol::CurveV2MetaPool, 
            hex!("6d0bd8365e2fcd0c2acf7d218f629a319b6c9d47").into(),
            vec![
                Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
                Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
                Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6")),
                Address::new(hex!("fd8e70e83E399307db3978D3F34B060a06792c36")),
            ]
        ).await;
    }

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_metapool2_discovery() {
        verify_discovery(
            hex!("59814dc53b4d415b68662433f6eea167ae64370432283598b5314b81a4801abb").into(), 
            Protocol::CurveV2MetaPool, 
            hex!("e60986759872393a8360a4a7abeab3a6e0ba7848").into(),
            vec![
                hex!("853d955acef822db058eb8505911ed77f175b99e").into(),
                hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").into(),
                hex!("466a756E9A7401B5e2444a3fCB3c2C12FBEa0a54").into(),
            ]
        ).await;
    }

    #[brontes_macros::test]
    async fn test_curve_crvUSD_metapool_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("01df49b92d7aa754862257bf2343ec44656a13ccf8f30bb6599d0dd267e477b8"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CurvecrvUSDPlainPool,
            pool_address: Address::new(hex!("9c3b46c0ceb5b9e304fcd6d88fc50f7dd24b31bc")),
            tokens:       vec![
                hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
                hex!("5E8422345238F34275888049021821E8E08CAa1f").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("9c3b46c0ceb5b9e304fcd6d88fc50f7dd24b31bc")),
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
    async fn test_curve_crypto_swap_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("b8225567ede93bc296b5ac263d5419f8910bc6c93554fbf5d7a643a945011743"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CurveCryptoSwapPool,
            pool_address: Address::new(hex!("97130cc28e99d13ce1ae41d022268b5cc7409cda")),
            tokens:       vec![
                hex!("81cb62d2cd9261f63a1ae96df715748dcbc97d46").into(),
                hex!("dac17f958d2ee523a2206206994597c13d831ec7").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("97130cc28e99d13ce1ae41d022268b5cc7409cda")),
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
    async fn test_curve_tri_crypto_discovery() {
        let utils = ClassifierTestUtils::new().await;
        let tx =
            B256::new(hex!("28359dab5f78b92fb89f826f37296d86174ff6c62b0e14b44ad8b6abd0de92da"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::CurveTriCryptoPool,
            pool_address: Address::new(hex!("84cecb5525c6b1c20070e742da870062e84da178")),
            tokens:       vec![
                hex!("a71d0588EAf47f12B13cF8eC750430d21DF04974").into(),
                hex!("b53ecF1345caBeE6eA1a65100Ebb153cEbcac40f").into(),
                hex!("f3b9569F82B18aEf890De263B84189bd33EBe452").into(),
            ],
        };

        utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("84cecb5525c6b1c20070e742da870062e84da178")),
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
