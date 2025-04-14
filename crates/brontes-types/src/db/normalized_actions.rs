use alloy_primitives::Address;
use clickhouse::DbRow;
use itertools::MultiUnzip;
use alloy_primitives::B256;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{normalized_actions::Action, GasDetails, Node, Root};

#[derive(Debug, Clone)]
pub struct TransactionRoot {
    pub block_number: u64,
    pub tx_hash:      B256,
    pub tx_idx:       usize,
    pub from_address: Address,
    pub to_address:   Option<Address>,
    pub gas_details:  GasDetails,
    pub trace_nodes:  Vec<TraceNode>,
}

impl From<(&Root<Action>, u64)> for TransactionRoot {
    fn from(value: (&Root<Action>, u64)) -> Self {
        let (root, block_number) = value;
        let tx_data = &root.data_store.0;
        let mut trace_nodes = Vec::new();
        make_trace_nodes(&root.head, tx_data, &mut trace_nodes);

        Self {
            from_address: root.get_from_address(),
            to_address: root.try_get_to_address(),
            block_number,
            tx_hash: root.tx_hash,
            tx_idx: root.position,
            gas_details: root.gas_details,
            trace_nodes,
        }
    }
}

impl Serialize for TransactionRoot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("TransactionRoot", 9)?;

        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("tx_idx", &self.tx_idx)?;
        ser_struct.serialize_field("from", &format!("{:?}", self.from_address))?;
        ser_struct
            .serialize_field("to", &self.to_address.as_ref().map(|addr| format!("{:?}", addr)))?;
        ser_struct.serialize_field(
            "gas_details",
            &(
                self.gas_details.coinbase_transfer,
                self.gas_details.priority_fee,
                self.gas_details.gas_used,
                self.gas_details.effective_gas_price,
            ),
        )?;

        let (trace_idx, trace_address, action_kind, action): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
            self.trace_nodes
                .iter()
                .map(|node| {
                    (
                        node.trace_idx,
                        node.trace_address.clone(),
                        node.action_kind,
                        node.action
                            .as_ref()
                            .map(|a| serde_json::to_string(a).unwrap()),
                    )
                })
                .multiunzip();

        ser_struct.serialize_field("trace_nodes.trace_idx", &trace_idx)?;
        ser_struct.serialize_field("trace_nodes.trace_address", &trace_address)?;
        ser_struct.serialize_field("trace_nodes.action_kind", &action_kind)?;
        ser_struct.serialize_field("trace_nodes.action", &action)?;

        ser_struct.end()
    }
}

impl DbRow for TransactionRoot {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "block_number",
        "tx_hash",
        "tx_idx",
        "from",
        "to",
        "gas_details",
        "trace_nodes.trace_idx",
        "trace_nodes.trace_address",
        "trace_nodes.action_kind",
        "trace_nodes.action",
    ];
}

fn make_trace_nodes(
    node: &Node,
    actions: &[Option<Vec<Action>>],
    trace_nodes: &mut Vec<TraceNode>,
) {
    trace_nodes.push((node, actions).into());

    for n in &node.inner {
        make_trace_nodes(n, actions, trace_nodes)
    }
}

#[derive(Debug, Clone)]
pub struct TraceNode {
    pub trace_idx:     u64,
    pub trace_address: Vec<u64>,
    pub action_kind:   Option<ActionKind>,
    pub action:        Option<Action>,
}

impl From<(&Node, &[Option<Vec<Action>>])> for TraceNode {
    fn from(value: (&Node, &[Option<Vec<Action>>])) -> Self {
        let (node, actions) = value;
        let action = actions
            .iter()
            .enumerate()
            .find(|(i, _)| *i == node.data)
            .and_then(|(_, a)| a.as_ref().and_then(|f| f.first()).cloned());
        Self {
            trace_idx: node.index,
            trace_address: node
                .trace_address
                .iter()
                .map(|i| *i as u64)
                .collect::<Vec<_>>()
                .clone(),
            action_kind: action.as_ref().map(Into::into),
            action,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
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

impl From<&Action> for ActionKind {
    fn from(value: &Action) -> Self {
        match value {
            Action::Swap(_) => ActionKind::Swap,
            Action::SwapWithFee(_) => ActionKind::SwapWithFee,
            Action::FlashLoan(_) => ActionKind::FlashLoan,
            Action::Batch(_) => ActionKind::Batch,
            Action::Mint(_) => ActionKind::Mint,
            Action::Burn(_) => ActionKind::Burn,
            Action::Transfer(_) => ActionKind::Transfer,
            Action::Liquidation(_) => ActionKind::Liquidation,
            Action::Collect(_) => ActionKind::Collect,
            Action::SelfDestruct(_) => ActionKind::SelfDestruct,
            Action::EthTransfer(_) => ActionKind::EthTransfer,
            Action::Unclassified(_) => ActionKind::Unclassified,
            Action::NewPool(_) => ActionKind::NewPool,
            Action::PoolConfigUpdate(_) => ActionKind::PoolConfigUpdate,
            Action::Aggregator(_) => ActionKind::Aggregator,
            Action::Revert => ActionKind::Revert,
        }
    }
}

impl Serialize for ActionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{:?}", self).serialize(serializer)
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::normalized_actions::{ActionKind, TransactionRoot},
        normalized_actions::Action,
        BlockTree,
    };

    async fn load_tree() -> Arc<BlockTree<Action>> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap().into()
    }

    #[brontes_macros::test]
    async fn test_into_tx_root() {
        let tree = load_tree().await;
        let root = &tree.clone().tx_roots[0];
        let tx_root = TransactionRoot::from((root, tree.header.number));

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
