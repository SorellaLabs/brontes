use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedNewPool, structured_trace::CallInfo};

pub mod lfj_v2_1 {
    use super::*;

    action_impl!(
        Protocol::LFJV2_1,
        crate::LFJFactory::createLBPairCall,
        NewPool,
        [LBPairCreated],
        logs: true,
    |info: CallInfo, log_data: LFJV2_1CreateLBPairCallLogs, _| {
        let logs = log_data.l_b_pair_created_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::LFJV2_1,
            pool_address: logs.LBPair,
            tokens: vec![logs.tokenX, logs.tokenY],
        })
    }
    );
}

pub mod lfj_v2_2 {
    use super::*;

    action_impl!(
            Protocol::LFJV2_2,
        crate::LFJV2_2Factory::createLBPairCall,
        NewPool,
        [LBPairCreated],
        logs: true,
        |info: CallInfo, log_data: LFJV2_2CreateLBPairCallLogs, _| {
            let logs = log_data.l_b_pair_created_field?;

            Ok(NormalizedNewPool {
                trace_index: info.trace_idx,
                protocol: Protocol::LFJV2_2,
                pool_address: logs.LBPair,
                tokens: vec![logs.tokenX, logs.tokenY],
            })
        }
    );
}
#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{normalized_actions::Action, TreeSearchBuilder};

    use super::*;

    #[brontes_macros::test]
    async fn test_lfj_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("feb3000cd801ad15204235813eab94004d697ccba75cc9e082dc96c5e63c1529"));

        let new_pool = Action::NewPool(NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::LFJ,
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
