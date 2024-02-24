use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::Protocol;

discovery_impl!(
    CompoundV2Discovery0,
    crate::CErc20Delegate::initialize_0Call,
    0x99ee778B9A6205657DD03B2B91415C8646d521ec,
    |deployed_address: Address, trace_index: u64, call_data: initialize_0Call, _| async move {
        // let token = call_data.;

        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::CompoundV2,
            tokens: vec![],
        }]
    }
);

discovery_impl!(
    CompoundV2Discovery1,
    crate::CErc20Delegate::initialize_1Call,
    0x99ee778B9A6205657DD03B2B91415C8646d521ec,
    |deployed_address: Address, trace_index: u64, call_data: initialize_1Call, _| async move {
        vec![NormalizedNewPool {
            pool_address: deployed_address,
            trace_index,
            protocol: Protocol::CompoundV2,
            tokens: vec![],
        }]
    }
);

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex, Address, B256};
    use brontes_types::{
        normalized_actions::{pool::NormalizedNewPool, Actions},
        tree::root::NodeData,
        Node, Protocol, TreeSearchArgs,
    };

    use crate::test_utils::ClassifierTestUtils;

    #[brontes_macros::test]
    async fn test_compound_v2_discovery() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let compound_v2_discovery =
            B256::from(hex!("d1a4bcb0999c7c236eba9817957fe39ab8b4f068fbada96ed1dd6982c3d45ea8"));

        let eq_action = Actions::NewPool(NormalizedNewPool {
            trace_index:  6,
            protocol:     Protocol::CompoundV2,
            pool_address: Address::from(hex!("4Ddc2D193948926D02f9B1fE9e1daa0718270ED5")),
            tokens:       vec![Address::from(hex!("4Ddc2D193948926D02f9B1fE9e1daa0718270ED5"))],
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node:  data
                .get_ref(node.data)
                .map(|a| a.is_new_pool())
                .unwrap_or_default(),
            child_node_to_collect: node
                .subactions
                .iter()
                .filter_map(|node| data.get_ref(*node))
                .any(|action| action.is_new_pool()),
        };

        classifier_utils
            .contains_action(compound_v2_discovery, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
