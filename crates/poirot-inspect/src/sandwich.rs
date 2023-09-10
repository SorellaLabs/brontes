use poirot_labeller::Labeller;
use crate::Inspector;
use poirot_types::{
    normalized_actions::{Actions, NormalizedAction},
    tree::{Node, Root, TimeTree},
};

use std::{collections::VecDeque, sync::Arc};

pub struct SandwichInspector {
    db: Arc<Labeller>,
}

//TODO: Sandwiching detection Algo:
// 1. Create a vec for each swap pair & index all swaps on that pair / contract
// 2. For each swap pair, check if more than 3 swaps. If so look for 2 swaps in opposite direction
//    from same addr
// 3. Check profitability of sandwich & index accordingly
#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {
        let mut roots: VecDeque<&Root<Actions>> =
            tree.roots.iter().collect::<Vec<&Root<Actions>>>().into();
        if roots.len() < 3 {
            return
        }

        let mut buffer: VecDeque<&Root<Actions>> = roots.drain(..3).collect();

        while buffer.len() > 2 {
            let first_node = buffer.pop_front().unwrap();
            let third_node = buffer.back().unwrap();
            if first_node.head.address != first_node.head.address {
                buffer.push_back(roots.pop_front().unwrap());
                continue
            }

            let second_node = buffer.get(buffer.len() - 2).unwrap();

            let _first_swaps = tree.inspect(first_node.tx_hash, Self::get_swaps);
            let _second_swaps = tree.inspect(second_node.tx_hash, Self::get_swaps);
            let _third_swaps = tree.inspect(third_node.tx_hash, Self::get_swaps);
        }
    }
}

impl SandwichInspector {
    fn get_swaps(node: &Node<Actions>) -> bool {
        node.data.get_action().is_swap()
    }
}
