use std::collections::{HashMap, HashSet};

use hex_literal::hex;
use poirot_core::{StaticReturnBindings, PROTOCOL_ADDRESS_MAPPING};
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::Actions,
    structured_trace::{TraceActions, TxTrace},
    tree::{Node, Root, TimeTree}
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, Header, H256, U256};
use reth_rpc_types::{
    trace::parity::{Action, TransactionTrace},
    Log
};

use crate::IntoAction;

const TRANSFER_TOPIC: H256 =
    H256(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

/// goes through and classifies all exchanges
#[derive(Debug)]
pub struct Classifier {
    known_dyn_protocols: HashMap<Address, (Address, Address)>,
    static_protocols:    HashMap<[u8; 4], Box<dyn IntoAction>>
}

impl Classifier {
    pub fn new(known_protocols: HashMap<[u8; 4], Box<dyn IntoAction>>) -> Self {
        Self { static_protocols: known_protocols, known_dyn_protocols: HashMap::default() }
    }

    pub fn build_tree(
        &mut self,
        traces: Vec<TxTrace>,
        header: Header,
        metadata: &Metadata
    ) -> TimeTree<Actions> {
        let roots = traces
            .into_par_iter()
            .map(|mut trace| {
                let logs = &trace.logs;
                let address = trace.trace[0].get_from_addr();
                let classification = self.classify_node(trace.trace.remove(0), logs);

                let node = Node {
                    inner: vec![],
                    index: 0,
                    finalized: !classification.is_unclassified(),
                    subactions: vec![],
                    address,
                    data: classification
                };

                let mut root = Root {
                    head:        node,
                    tx_hash:     trace.tx_hash,
                    private:     false,
                    gas_details: poirot_types::tree::GasDetails {
                        coinbase_transfer:   None,
                        gas_used:            trace.gas_used,
                        effective_gas_price: trace.effective_price,
                        priority_fee:        trace.effective_price
                            - header.base_fee_per_gas.unwrap()
                    }
                };

                for (index, trace) in trace.trace.into_iter().enumerate() {
                    root.gas_details.coinbase_transfer =
                        self.get_coinbase_transfer(header.beneficiary, &trace.action);

                    let address = trace.get_from_addr();
                    let classification = self.classify_node(trace, logs);
                    let node = Node {
                        index: (index + 1) as u64,
                        inner: vec![],
                        finalized: !classification.is_unclassified(),
                        subactions: vec![],
                        address,
                        data: classification
                    };

                    root.insert(node.address, node);
                }

                root
            })
            .collect::<Vec<Root<Actions>>>();

        let mut tree = TimeTree {
            roots,
            header,
            eth_prices: metadata.eth_prices.clone(),
            avg_priority_fee: 0
        };

        self.try_classify_unknown(&mut tree);

        // remove duplicate swaps
        tree.remove_duplicate_data(
            |node| node.data.is_swap(),
            |other_nodes, node| {
                let Actions::Swap(swap_data) = &node.data else { unreachable!() };
                other_nodes
                    .into_iter()
                    .filter_map(|(index, data)| {
                        let Actions::Transfer(transfer) = data else { return None };
                        if transfer.amount == swap_data.amount_in
                            && transfer.token == swap_data.token_in
                        {
                            return Some(*index)
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            },
            |node| (node.index, node.data.clone())
        );

        // remove duplicate mints
        tree.remove_duplicate_data(
            |node| node.data.is_mint(),
            |other_nodes, node| {
                let Actions::Mint(mint_data) = &node.data else { unreachable!() };
                other_nodes
                    .into_iter()
                    .filter_map(|(index, data)| {
                        let Actions::Transfer(transfer) = data else { return None };
                        for (amount, token) in mint_data.amount.iter().zip(&mint_data.token) {
                            if transfer.amount.eq(amount) && transfer.token.eq(token) {
                                return Some(*index)
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            },
            |node| (node.index, node.data.clone())
        );

        tree.finalize_tree();

        tree
    }

    fn get_coinbase_transfer(&self, builder: Address, action: &Action) -> Option<U256> {
        match action {
            Action::Call(action) => {
                if action.to == builder {
                    return Some(action.value)
                }
                None
            }
            _ => None
        }
    }

    fn classify_node(&self, trace: TransactionTrace, logs: &Vec<Log>) -> Actions {
        let address = trace.get_from_addr();

        if let Some(mapping) = PROTOCOL_ADDRESS_MAPPING.get(format!("{address}").as_str()) {
            let calldata = trace.get_calldata();
            let return_bytes = trace.get_return_calldata();
            let sig = &calldata[0..4];
            let res: StaticReturnBindings = mapping.try_decode(&calldata).unwrap();

            return self.static_protocols.get(sig).unwrap().decode_trace_data(
                res,
                return_bytes,
                address,
                logs
            )
        } else {
            let rem = logs
                .iter()
                .filter(|log| log.address == address)
                .cloned()
                .collect::<Vec<Log>>();

            if rem.len() == 1 {
                if let Some((addr, from, to, value)) = self.decode_transfer(&rem[0]) {
                    return Actions::Transfer(poirot_types::normalized_actions::NormalizedTransfer {
                        to,
                        from,
                        token: addr,
                        amount: value
                    })
                }
            }

            Actions::Unclassified(trace, rem)
        }
    }

    /// tries to prove dyn mint, dyn burn and dyn swap.
    fn prove_dyn_action(
        &self,
        node: &mut Node<Actions>,
        token_0: Address,
        token_1: Address
    ) -> Option<Actions> {
        let addr = node.address;
        let subactions = node.get_all_sub_actions();
        let logs = subactions
            .iter()
            .flat_map(|i| i.get_logs())
            .collect::<Vec<_>>();

        let mut transfer_data = Vec::new();

        // index all transfers. due to tree this should only be two transactions
        for log in logs {
            if let Some((token, from, to, value)) = self.decode_transfer(&log) {
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
                        from:   from0,
                        token:  vec![t0, t1],
                        amount: vec![value0, value1]
                    }))
                }
                // mint
                else {
                    return Some(Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
                        to:     to0,
                        token:  vec![t0, t1],
                        amount: vec![value0, value1]
                    }))
                }
            }
            // if to0 is to our addr then its the out token
            if to0 == addr {
                return Some(Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    call_address: addr,
                    token_in:     t1,
                    token_out:    t0,
                    amount_in:    value1,
                    amount_out:   value0
                }))
            } else {
                return Some(Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    call_address: addr,
                    token_in:     t0,
                    token_out:    t1,
                    amount_in:    value0,
                    amount_out:   value1
                }))
            }
        }
        // pure mint and burn
        if transfer_data.len() == 1 {
            let (token, from, to, value) = transfer_data.remove(0);
            if from == addr {
                return Some(Actions::Mint(poirot_types::normalized_actions::NormalizedMint {
                    to,
                    token: vec![token],
                    amount: vec![value]
                }))
            } else {
                return Some(Actions::Burn(poirot_types::normalized_actions::NormalizedBurn {
                    from,
                    token: vec![token],
                    amount: vec![value]
                }))
            }
        }

        None
    }

    fn decode_transfer(&self, log: &Log) -> Option<(Address, Address, Address, U256)> {
        if log.topics.get(0).eq(&Some(&TRANSFER_TOPIC)) {
            let from = Address::from_slice(&log.data[11..31]);
            let to = Address::from_slice(&log.data[41..63]);
            let data = U256::try_from_be_slice(&log.data[64..]).unwrap();
            return Some((log.address, from, to, data))
        }

        None
    }

    /// checks to see if we have a direct to <> from mapping for underlying
    /// transfers
    fn is_possible_exchange(&self, actions: Vec<Actions>) -> bool {
        let mut to_address = HashSet::new();
        let mut from_address = HashSet::new();

        for action in &actions {
            if let Actions::Transfer(t) = action {
                to_address.insert(t.to);
                from_address.insert(t.from);
            }
        }

        for to_addr in to_address {
            if from_address.contains(&to_addr) {
                return true
            }
        }

        false
    }

    /// tries to classify new exchanges
    fn try_clasify_exchange(
        &self,
        node: &mut Node<Actions>
    ) -> Option<(Address, (Address, Address), Actions)> {
        let addr = node.address;
        let subactions = node.get_all_sub_actions();
        let logs = subactions
            .iter()
            .flat_map(|i| i.get_logs())
            .collect::<Vec<_>>();

        let mut transfer_data = Vec::new();

        // index all transfers. due to tree this should only be two transactions
        for log in logs {
            if let Some((token, from, to, value)) = self.decode_transfer(&log) {
                // if tokens don't overlap and to & from don't overlap
                if from != addr && to != addr {
                    continue
                }

                transfer_data.push((token, from, to, value));
            }
        }

        // isn't an exchange
        if transfer_data.len() != 2 {
            return None
        }

        let (t0, from0, to0, value0) = transfer_data.remove(0);
        let (t1, from1, to1, value1) = transfer_data.remove(1);

        // is a exchange
        if t0 != t1
            && (from0 == addr || to0 == addr)
            && (from1 == addr || to1 == addr)
            && (from0 != from1)
        {
            let swap = if t0 == addr {
                Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    call_address: addr,
                    token_in:     t1,
                    token_out:    t0,
                    amount_in:    value1,
                    amount_out:   value0
                })
            } else {
                Actions::Swap(poirot_types::normalized_actions::NormalizedSwap {
                    call_address: addr,
                    token_in:     t0,
                    token_out:    t1,
                    amount_in:    value0,
                    amount_out:   value1
                })
            };
            return Some((addr, (t0, t1), swap))
        }

        None
    }

    fn try_classify_unknown(&mut self, tree: &mut TimeTree<Actions>) {
        let new_classifed_exchanges = tree.dyn_classify(
            |address, sub_actions| {
                // we can dyn classify this shit
                if PROTOCOL_ADDRESS_MAPPING.contains_key(format!("{address}").as_str()) {
                    // this is already classified
                    return false
                }
                if self.known_dyn_protocols.contains_key(&address) {
                    return true
                } else if self.is_possible_exchange(sub_actions) {
                    return true
                }

                false
            },
            |node| {
                if self.known_dyn_protocols.contains_key(&node.address) {
                    let (token_0, token_1) = self.known_dyn_protocols.get(&node.address).unwrap();
                    if let Some(res) = self.prove_dyn_action(node, *token_0, *token_1) {
                        // we have reduced the lower part of the tree. we can delete this now
                        node.inner.clear();
                        node.data = res;
                    }
                } else if let Some((ex_addr, tokens, action)) = self.try_clasify_exchange(node) {
                    node.inner.clear();
                    node.data = action;

                    return Some((ex_addr, tokens))
                }
                None
            }
        );

        new_classifed_exchanges.into_iter().for_each(|(k, v)| {
            self.known_dyn_protocols.insert(k, v);
        });
    }
}
