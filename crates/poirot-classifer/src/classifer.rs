use crate::IntoAction;
use hex_literal::hex;
use poirot_core::{StaticBindings, StaticReturnBindings, PROTOCOL_ADDRESS_MAPPING};
use poirot_types::{
    normalized_actions::Actions,
    structured_trace::{TraceActions, TxTrace},
    tree::{Node, Root, TimeTree},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{keccak256, Address, Log, H256, U256};
use reth_rpc_types::trace::parity::TransactionTrace;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

const TRANSFER_TOPIC: H256 =
    H256(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

/// goes through and classifies all exchanges
pub struct Classifier {
    known_dyn_exchanges: HashMap<Address, (Address, Address)>,
    static_exchanges: HashMap<[u8; 4], Box<dyn IntoAction>>,
}

impl Classifier {
    pub fn build_tree(&mut self, traces: Vec<TxTrace>) -> TimeTree<Actions> {
        let roots = traces
            .into_par_iter()
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
        tree.freeze_tree();

        tree
    }

    fn classify_node(&self, trace: TransactionTrace, logs: &Vec<Log>) -> Actions {
        let address = trace.get_from_addr();

        if let Some(mapping) = PROTOCOL_ADDRESS_MAPPING.get(format!("{address}").as_str()) {
            let calldata = trace.get_calldata();
            let return_bytes = trace.get_return_calldata();
            let sig = &calldata[0..3];
            let res: StaticReturnBindings = mapping.try_decode(&calldata).unwrap();

            return self.static_exchanges.get(sig).unwrap().decode_trace_data(res, return_bytes)
        } else {
            let rem =
                logs.iter().filter(|log| log.address == address).cloned().collect::<Vec<Log>>();
            return Actions::Unclassified(trace, rem)
        }
    }

    /// tries to prove dyn mint, dyn burn and dyn swap.
    fn prove_dyn_action(
        &self,
        node: &mut Node<Actions>,
        token_0: Address,
        token_1: Address,
    ) -> Option<Actions> {
        let addr = node.address;
        let subactions = node.get_all_sub_actions();
        let logs = subactions.iter().flat_map(|i| i.get_logs()).collect::<Vec<_>>();

        let mut transfer_data = Vec::new();

        // index all transfers. due to tree this should only be two transactions
        for log in logs {
            if let Some((token, from, to, value)) = self.decode_transfer(log) {
                // if tokens don't overlap and to & from don't overlap
                if (token_0 != token && token_1 != token) || (from != addr && to != addr) {
                    continue
                }

                transfer_data.push((token, from, to, value));
            }
        }

        if transfer_data.len() == 2 {
            let (t0, from0, to0, value0) = transfer_data.remove(0);
            let (t1, from1, to1, value1) = transfer_data.remove(1);

            // sending 2 transfers to same addr
            if to0 == to1 && from0 == from1 {
                // burn
                if to0 == node.address {
                    return Some(Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
                        from: vec![from0, from1],
                        to: vec![to0, to1],
                        token: vec![t0, t1],
                        amount: vec![value0, value1],
                    }))
                }
                // mint
                else {
                    return Some(Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
                        from: vec![from0, from1],
                        to: vec![to0, to1],
                        token: vec![t0, t1],
                        amount: vec![value0, value1],
                    }))
                }
            }
            // if to0 is to our addr then its the out token
            if to0 == addr {
                return Some(Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    address: addr,
                    token_in: t1,
                    token_out: t0,
                    amount_in: value1,
                    amount_out: value0,
                }))
            } else {
                return Some(Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    address: addr,
                    token_in: t0,
                    token_out: t1,
                    amount_in: value0,
                    amount_out: value1,
                }))
            }
        }
        // pure mint and burn
        if transfer_data.len() == 1 {
            let (token, from, to, value) = transfer_data.remove(0);
            if from == addr {
                return Some(Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
                    from: vec![from],
                    to: vec![to],
                    token: vec![token],
                    amount: vec![value],
                }))
            } else {
                return Some(Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
                    from: vec![from],
                    to: vec![to],
                    token: vec![token],
                    amount: vec![value],
                }))
            }
        }

        None
    }

    fn decode_transfer(&self, log: Log) -> Option<(Address, Address, Address, U256)> {
        if log.topics.get(0).eq(&Some(&TRANSFER_TOPIC)) {
            let from = Address::from_slice(&log.data[11..31]);
            let to = Address::from_slice(&log.data[41..63]);
            let data = U256::try_from_be_slice(&log.data[64..]).unwrap();
            return Some((log.address, from, to, data))
        }

        None
    }

    fn is_possible_action(&self, node_addr: Address, actions: Vec<Actions>) -> bool {
        // let sub_address = actions.iter().map(|action| action.
        false
    }

    fn try_classify_unknown(&mut self, tree: &mut TimeTree<Actions>) {
        tree.dyn_classify(
            |address, sub_actions| {
                // we can dyn classify this shit
                if self.known_dyn_exchanges.contains_key(&address) {
                    return true
                } else {
                    if self.is_possible_action(address, sub_actions) {
                        return true
                    };
                }

                return false
            },
            |node| {
                if self.known_dyn_exchanges.contains_key(&node.address) {
                    let (token_0, token_1) = self.known_dyn_exchanges.get(&node.address).unwrap();
                    if let Some(res) = self.prove_dyn_action(node, *token_0, *token_1) {
                        // we have reduced the lower part of the tree. we can delete this now
                        node.inner.clear();
                        node.data = res;
                    }
                    return
                } else {
                    // try to classify, else yoink
                    //
                }

                // false
            },
        );
    }
}
