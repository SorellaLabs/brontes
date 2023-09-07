use poirot_types::{
    structured_trace::StructuredTrace,
    tree::{Node, Root, TimeTree},
};
use std::collections::HashSet;

use poirot_core::PROTOCOL_ADDRESS_MAPPING;
use poirot_types::{normalized_actions::Actions, structured_trace::TxTrace};
use reth_primitives::{Address, Log};

/// goes through and classifies all exchanges
pub struct Classifier {
    known_dyn_exchanges: HashSet<Address>,
}

impl Classifier {
    pub fn build_tree(&mut self, traces: Vec<TxTrace>) -> TimeTree<Actions> {
        let roots = traces
            .into_iter()
            .map(|mut trace| {
                let logs = &trace.logs;
                let node = Node {
                    inner: vec![],
                    frozen: false,
                    subactions: vec![],
                    address: trace.trace[0].get_from_addr(),
                    data: self.classify_node(trace.trace.remove(0), logs),
                };
                let mut root = Root { head: node, tx_hash: trace.tx_hash };

                for trace in trace.trace {
                    let node = Node {
                        inner: vec![],
                        frozen: false,
                        subactions: vec![],
                        address: trace.get_from_addr(),
                        data: self.classify_node(trace, logs),
                    };
                    root.insert(node.address, node);
                }

                root
            })
            .collect::<Vec<Root<Actions>>>();

        let mut tree = TimeTree { roots };
        self.try_classify_unknown(&mut tree);

        tree
    }

    fn classify_node(&self, trace: StructuredTrace, logs: &Vec<Log>) -> Actions {
        let address = trace.get_from_addr();
        if let Some(known_mapping) = PROTOCOL_ADDRESS_MAPPING.get(format!("{address}").as_str()) {
            todo!()
        } else {
            let rem =
                logs.iter().filter(|log| log.address == address).cloned().collect::<Vec<Log>>();
            return Actions::Unclassified(trace, rem)
        }
    }

    fn try_classify_unknown(&mut self, tree: &mut TimeTree<Actions>) {
        tree.map(|node| {
            // let addresses = node.
            if self.known_dyn_exchanges.contains(&node.address) {
            } else {
                // try to classify, else yoink
            }

            // false
            true
        });
    }
}
