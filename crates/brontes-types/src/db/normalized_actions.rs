use reth_primitives::B256;
use serde::{Deserialize, Serialize};

use crate::{normalized_actions::Actions, GasDetails, Node, Root};

pub struct TransactionRoot {
    pub tx_hash:     B256,
    pub tx_idx:      usize,
    pub gas_details: GasDetails,
    pub trace_nodes: Vec<TransactionNode>,
}

impl From<&Root<Actions>> for TransactionRoot {
    fn from(value: &Root<Actions>) -> Self {
        let tx_data = &value.data_store.0;
        let mut trace_nodes = Vec::new();
        make_trace_nodes(&value.head, &tx_data, &mut trace_nodes);

        Self {
            tx_hash: value.tx_hash,
            tx_idx: value.position,
            gas_details: value.gas_details,
            trace_nodes,
        }
    }
}

fn make_trace_nodes(
    node: &Node,
    actions: &[Option<Actions>],
    trace_nodes: &mut Vec<TransactionNode>,
) {
    trace_nodes.push((node, actions).into());

    for n in &node.inner {
        make_trace_nodes(&n, actions, trace_nodes)
    }
}

pub struct TransactionNode {
    pub trace_idx:     u64,
    pub trace_address: Vec<usize>,
    pub action_kind:   Option<ActionKind>,
    pub action:        Option<Actions>,
}

impl From<(&Node, &[Option<Actions>])> for TransactionNode {
    fn from(value: (&Node, &[Option<Actions>])) -> Self {
        let (node, actions) = value;
        let action = actions
            .iter()
            .enumerate()
            .find(|(i, _)| *i == node.data)
            .map(|(_, a)| a)
            .cloned()
            .flatten();
        Self {
            trace_idx: node.index,
            trace_address: node.trace_address.clone(),
            action_kind: action.as_ref().map(Into::into),
            action,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    Swap,
    SwapWithFee,
    FlashLoan,
    Batch,
    Transfer,
    Mint,
    Burn,
    Collect,
    Liquidation,
    Unclassified,
    SelfDestruct,
    EthTransfer,
    NewPool,
    PoolConfigUpdate,
    Aggregator,
    Revert,
}

impl From<&Actions> for ActionKind {
    fn from(value: &Actions) -> Self {
        match value {
            Actions::Swap(_) => ActionKind::Swap,
            Actions::SwapWithFee(_) => ActionKind::SwapWithFee,
            Actions::FlashLoan(_) => ActionKind::FlashLoan,
            Actions::Batch(_) => ActionKind::Batch,
            Actions::Mint(_) => ActionKind::Mint,
            Actions::Burn(_) => ActionKind::Burn,
            Actions::Transfer(_) => ActionKind::Transfer,
            Actions::Liquidation(_) => ActionKind::Liquidation,
            Actions::Collect(_) => ActionKind::Collect,
            Actions::SelfDestruct(_) => ActionKind::SelfDestruct,
            Actions::EthTransfer(_) => ActionKind::EthTransfer,
            Actions::Unclassified(_) => ActionKind::Unclassified,
            Actions::NewPool(_) => ActionKind::NewPool,
            Actions::PoolConfigUpdate(_) => ActionKind::PoolConfigUpdate,
            Actions::Aggregator(_) => ActionKind::Aggregator,
            Actions::Revert => ActionKind::Revert,
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{normalized_actions::Actions, BlockTree, TreeSearchBuilder};

    use super::*;

    async fn load_tree() -> Arc<BlockTree<Actions>> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap().into()
    }

    #[brontes_macros::test]
    async fn test_into_tx_root() {
        let tx = &hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        let tree: Arc<BlockTree<Actions>> = load_tree().await;

        let burns = tree
            .clone()
            .collect(tx, TreeSearchBuilder::default().with_action(Actions::is_burn))
            .collect::<Vec<_>>();
        assert_eq!(burns.len(), 1);
        let swaps = tree
            .collect(tx, TreeSearchBuilder::default().with_action(Actions::is_swap))
            .collect::<Vec<_>>();
        assert_eq!(swaps.len(), 3);

        let root = &tree.tx_roots[0];

        let tx_root: TransactionRoot = (&root).into();

        let burns = tx_root
            .trace_nodes
            .iter()
            .filter_map(|node| node.action_kind)
            .filter(|action| matches!(action, ActionKind::Burn))
            .count();
        assert_eq!(burns, 1);

        let swaps = tx_root
            .trace_nodes
            .iter()
            .filter_map(|node| node.action_kind)
            .filter(|action| matches!(action, ActionKind::Swap))
            .count();
        assert_eq!(swaps, 3);
    }
}
