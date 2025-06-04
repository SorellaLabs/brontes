use std::sync::Arc;

use alloy_primitives::Address;
use brontes_macros::{action_impl, discovery_impl};
use brontes_pricing::{make_call_request, Protocol};
use brontes_types::{
    normalized_actions::NormalizedNewPool, structured_trace::CallInfo, traits::TracingProvider,
};
use brontes_types::constants::PENDLE_V2_SY_ASSETS_API_URL;
use serde::{Deserialize, Serialize};

discovery_impl!(
    PendleV2Discovery,
    crate::PendleMarketV3Factory::createNewMarketCall,
    0xd29e76c6F15ada0150D10A1D3f45aCCD2098283B,
    |deployed_address: Address, trace_index: u64, _: createNewMarketCall, tracer: Arc<T>| async move {
        parse_market_pool(Protocol::PendleV2, deployed_address, trace_index, tracer).await
    } // sy pt yt
);

action_impl!(
    Protocol::PendleV2,
    crate::PendleYieldContractFactory::createYieldContractCall,
    NewPool,
    [CreateYieldContract],
    logs:true,
    |info: CallInfo, log_data: PendleV2CreateYieldContractCallLogs, _| {
        let logs=log_data.create_yield_contract_field?;
        // make std for pt addr due to expiry params with sy
        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::PendleV2,
            pool_address: logs.YT,
            tokens: vec![logs.SY, logs.PT, logs.YT],
        })
    }
);

alloy_sol_types::sol!(
    function readTokens() external view returns (address,address,address);
    function getTokensIn() external view returns(address[]);
    function getTokensOut() external view returns(address[]);
);

pub async fn query_pendle_v2_sy_underlying_tokens<T: TracingProvider>(
    tracer: &Arc<T>,
    sy_token: &Address,
) -> Vec<Address> {
    let mut result = Vec::new();
    if let Ok(call_return) = make_call_request(getTokensInCall {}, tracer, *sy_token, None).await {
        result.extend(call_return._0);
    }
    if let Ok(call_return) = make_call_request(getTokensOutCall {}, tracer, *sy_token, None).await {
        result.extend(call_return._0);
    }
    result.sort_unstable();
    result.dedup();
    result
}

pub async fn query_pendle_v2_market_tokens<T: TracingProvider>(
    tracer: &Arc<T>,
    market: &Address,
) -> Vec<Address> {
    let mut result = Vec::new();
    if let Ok(call_return) = make_call_request(readTokensCall {}, tracer, *market, None).await {
        result.push(call_return._0);
        result.push(call_return._1);
        result.push(call_return._2);
    }
    result
}

async fn parse_market_pool<T: TracingProvider>(
    protocol: Protocol,
    deployed_address: Address,
    trace_index: u64,
    tracer: Arc<T>,
) -> Vec<NormalizedNewPool> {
    let tokens = query_pendle_v2_market_tokens(&tracer, &deployed_address).await;

    vec![NormalizedNewPool { trace_index, protocol, pool_address: deployed_address, tokens }]
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PendleAsset {
    pub name: String,
    pub decimals: u8,
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Address,
    pub symbol: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pro_icon: Option<String>,
}

fn deserialize_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize)]
pub struct PendleAssetsResponse {
    pub assets: Vec<PendleAsset>,
}

pub async fn get_pendle_v2_sy_pools<T: TracingProvider>(tracer: &Arc<T>) -> Result<Vec<NormalizedNewPool>, Box<dyn std::error::Error>> {
    let url = PENDLE_V2_SY_ASSETS_API_URL;
    
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    
    let pendle_response: PendleAssetsResponse = response.json().await?;
    
    // Filter for SY assets only
    let sy_assets: Vec<PendleAsset> = pendle_response
        .assets
        .into_iter()
        .filter(|asset| asset.tags.contains(&"SY".to_string()))
        .collect();

    let underlying_tokens: Vec<_> = futures::future::join_all(
        sy_assets.iter().map(|asset| query_pendle_v2_sy_underlying_tokens(&tracer, &asset.address))
    ).await.into_iter().collect();

    let pools: Vec<_> = sy_assets.iter().zip(underlying_tokens.iter()).map(|(asset, tokens)| {
        NormalizedNewPool {
            trace_index: 0,
            protocol: Protocol::PendleV2,
            pool_address: asset.address,
            tokens: {
                let mut combined_tokens = vec![asset.address.clone()];
                combined_tokens.extend(tokens);
                combined_tokens
            }
        }
    }).collect();
    
    
    Ok(pools)
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{normalized_actions::Action, TreeSearchBuilder};

    use super::*;

    #[brontes_macros::test]
    async fn test_pendle_v2_new_market_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("620f07fc5d7781598214e2524b8c226ae8e475ec422fdad1272ab2775a80bf0a"));

        let new_pool = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::PendleV2,
            pool_address: Address::new(hex!("0f5814de3581cb1d8ad2b608d6ef2e6409738c36")),
            tokens:       vec![
                Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
                Address::new(hex!("9aFa9999e45484Adf5d8EED8D9Dfe0693BACd838")),
            ],
        });

        classifier_utils
            .contains_action(
                tx,
                0,
                new_pool,
                TreeSearchBuilder::default().with_action(Action::is_new_pool),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_pendle_v2_yield_contract_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("feb3000cd801ad15204235813eab94004d697ccba75cc9e082dc96c5e63c1529"));

        let new_pool = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::PendleV2,
            pool_address: Address::new(hex!("ea2c9470aec6251ef10a28d783ab877d17706bc4")),
            tokens:       vec![
                Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
                Address::new(hex!("99ea4dB9EE77ACD40B119BD1dC4E33e1C070b80d")),
            ],
        });

        classifier_utils
            .contains_action(
                tx,
                0,
                new_pool,
                TreeSearchBuilder::default().with_action(Action::is_new_pool),
            )
            .await
            .unwrap();
    }

}
