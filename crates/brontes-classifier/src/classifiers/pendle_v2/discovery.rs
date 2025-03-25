use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedNewPool, structured_trace::CallInfo};

action_impl!(
    Protocol::PendleV2,
    crate::PendleMarketV3Factory::createNewMarketCall,
    NewPool,
    [CreateNewMarket],
    logs: true,
    |info: CallInfo, log_data: PendleV2CreateNewMarketCallLogs, db_tx: &DB| {
        let logs = log_data.create_new_market_field?;

        let details=db_tx.get_protocol_details(logs.PT)?;
        let SY=details.token0;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::PendleV2,
            pool_address: logs.market,
            tokens: vec![logs.PT, SY],
        })
    }
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
            pool_address: logs.PT,
            tokens: vec![logs.SY, logs.YT],
        })
    }
);

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
