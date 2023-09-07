use std::collections::HashSet;

use poirot_core::PROTOCOL_ADDRESS_MAPPING;
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use reth_primitives::Address;

/// goes through and classifies all exchanges
pub struct Classifier {
    known_dyn_exchanges: HashSet<Address>,
}

impl Classifier {
    pub fn classify_tree(&mut self, mut tree: TimeTree<Actions>) {
        tree.map(|node| {
            // let addresses = node.
            if let Some(protocol_name) =
                PROTOCOL_ADDRESS_MAPPING.get(format!("{}", node.address).as_str())
            {
            } else if self.known_dyn_exchanges.contains(&node.address) {
            } else {
                // try to classify, else yoink
            }
            // if nodes.
            true
        });
    }
}
