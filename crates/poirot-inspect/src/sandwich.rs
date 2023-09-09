use super::database::InspectorDataClient;
use crate::Inspector;
use poirot_types::{
    normalized_actions::{Actions, NormalizedAction},
    tree::{Node, Root, TimeTree},
};
use reth_primitives::H256;
use std::{collections::VecDeque, sync::Arc};

pub struct SandwichInspector {
    db: Arc<InspectorDataClient>,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {
        let mut roots: VecDeque<&Root<Actions>> =
            tree.roots.iter().map(|n| n).collect::<Vec<&Root<Actions>>>().into();
        if roots.len() < 3 {
            return
        }

        let mut buffer: VecDeque<&Root<Actions>> = roots.drain(..3).collect();

        while buffer.len() > 2 {
            let first_node = buffer.pop_front().unwrap();
            let third_node = buffer.get(buffer.len() - 1).unwrap();
            if first_node.head.address != first_node.head.address {
                buffer.push_back(roots.pop_front().unwrap());
                continue
            }

            let second_node = buffer.get(buffer.len() - 2).unwrap();

            let first_swaps = tree.inspect(first_node.tx_hash, |node| Self::get_swaps(node));
            let second_swaps = tree.inspect(second_node.tx_hash, |node| Self::get_swaps(node));
            let third_swaps = tree.inspect(third_node.tx_hash, |node| Self::get_swaps(node));
        }
    }
}

impl SandwichInspector {
    fn get_swaps(node: &Node<Actions>) -> bool {
        node.data.get_action().is_swap()
    }
}
