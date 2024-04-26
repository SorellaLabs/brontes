use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    DodoDVMDiscovery,
    crate::DodoDVMFactory::createDODOVendingMachineCall,
    0x72d220ce168c4f361dd4dee5d826a01ad8598f6c,
    |deployed_address: Address, trace_index: u64, call_data: createDODOVendingMachineCall, _| async move {
        let base_token = call_data.baseToken;
        let quote_token = call_data.quoteToken;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::Dodo,
            tokens: vec![base_token, quote_token],
        }]
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDVMFactory::createDODOVendingMachineCall,
    NewPool,
    [NewDVM],
    logs: true,
    |info: CallInfo, log_data: DodoDODOVendingMachinelCallLogs, _| {
        let logs = log_data.pool_registered_field?;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::BalancerV2,
            pool_address: logs.poolAddress,
            tokens: vec![],
        })
    }
);


discovery_impl!(
    DodoDSPDiscovery,
    crate::DodoDSPFactory::createDODOStablePoolCall,
    0x6fdDB76c93299D985f4d3FC7ac468F9A168577A4,
    |deployed_address: Address, trace_index: u64, call_data: createDODOStablePoolCall, _| async move {
        let base_token = call_data.baseToken;
        let quote_token = call_data.quoteToken;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::Dodo,
            tokens: vec![base_token, quote_token],
        }]
    }
);


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::{Actions, NormalizedNewPool}, Protocol::UniswapV3,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_dodo_dvm_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("620f07fc5d7781598214e2524b8c226ae8e475ec422fdad1272ab2775a80bf0a"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::Dodo,
            pool_address: Address::new(hex!("1FA0d58e663017cdd80B87fd24C46818364fc9B6")),
            tokens:       vec![Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")), Address::new(hex!("9aFa9999e45484Adf5d8EED8D9Dfe0693BACd838"))],
        };
            
        classifier_utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("0f5814de3581cb1d8ad2b608d6ef2e6409738c36")),
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
    async fn test_dodo_dsp_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx =
            B256::from(hex!("feb3000cd801ad15204235813eab94004d697ccba75cc9e082dc96c5e63c1529"));

        let eq_create = NormalizedNewPool {
            trace_index:  1,
            protocol:     Protocol::Dodo,
            pool_address: Address::new(hex!("1FA0d58e663017cdd80B87fd24C46818364fc9B6")),
            tokens:       vec![Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")), Address::new(hex!("99ea4dB9EE77ACD40B119BD1dC4E33e1C070b80d"))],
        };
    
        classifier_utils
            .test_discovery_classification(
                tx,
                Address::new(hex!("ea2c9470aec6251ef10a28d783ab877d17706bc4")),
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