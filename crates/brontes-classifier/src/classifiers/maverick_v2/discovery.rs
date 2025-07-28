use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedNewPool, structured_trace::CallInfo};

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Factory::create_0Call,
    NewPool,
    [PoolCreated],
    logs: true,
    |info: CallInfo, log_data: MaverickV2Create_0CallLogs, _| {
        let logs = log_data.pool_created_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::MaverickV2,
            pool_address: logs.poolAddress,
            tokens: vec![logs.tokenA, logs.tokenB],
        })
    }
);

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Factory::createPermissioned_0Call,
    NewPool,
    [PoolCreated],
    logs: true,
    |info: CallInfo, log_data: MaverickV2CreatePermissioned_0CallLogs, _| {
        let logs = log_data.pool_created_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::MaverickV2,
            pool_address: logs.poolAddress,
            tokens: vec![logs.tokenA, logs.tokenB],
        })
    }
);

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Factory::create_1Call,
    NewPool,
    [PoolCreated],
    logs: true,
    |info: CallInfo, log_data: MaverickV2Create_1CallLogs, _| {
        let logs = log_data.pool_created_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::MaverickV2,
            pool_address: logs.poolAddress,
            tokens: vec![logs.tokenA, logs.tokenB],
        })
    }
);

action_impl!(
    Protocol::MaverickV2,
    crate::MaverickV2Factory::createPermissioned_1Call,
    NewPool,
    [PoolCreated],
    logs: true,
    |info: CallInfo, log_data: MaverickV2CreatePermissioned_1CallLogs, _| {
        let logs = log_data.pool_created_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::MaverickV2,
            pool_address: logs.poolAddress,
            tokens: vec![logs.tokenA, logs.tokenB],
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
    async fn test_maverick_v2_asymmetric_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("620f07fc5d7781598214e2524b8c226ae8e475ec422fdad1272ab2775a80bf0a"));

        let new_pool = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::MaverickV2,
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
    async fn test_maverick_v2_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("feb3000cd801ad15204235813eab94004d697ccba75cc9e082dc96c5e63c1529"));

        let new_pool = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::MaverickV2,
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
