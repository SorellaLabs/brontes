use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use brontes_database::Metadata;
use brontes_types::{
    normalized_actions::{
        Actions, NormalizedBurn, NormalizedMint, NormalizedSwap, NormalizedTransfer,
    },
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    tree::{GasDetails, Node, Root, TimeTree},
};
use hex_literal::hex;
use itertools::Itertools;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use reth_primitives::{Address, Header, H160, H256, U256};
use reth_rpc_types::{trace::parity::Action, Log};
use tracing::{debug, info};

use crate::{StaticReturnBindings, PROTOCOL_ADDRESS_MAPPING};

const TRANSFER_TOPIC: H256 =
    H256(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

/// goes through and classifies all exchanges
#[derive(Debug)]
// read write lock
pub struct Classifier {
    pub known_dyn_protocols: RwLock<HashMap<Address, (Address, Address)>>,
}

impl Classifier {
    pub fn new() -> Self {
        Self { known_dyn_protocols: RwLock::new(HashMap::default()) }
    }

    pub fn build_tree(
        &self,
        traces: Vec<TxTrace>,
        header: Header,
        metadata: &Metadata,
    ) -> TimeTree<Actions> {
        let roots = traces
            .into_par_iter()
            .filter_map(|mut trace| {
                if trace.trace.is_empty() {
                    return None
                }

                let root_trace = trace.trace[0].clone();
                let address = root_trace.get_from_addr();
                let classification = self.classify_node(trace.trace.remove(0), 0);

                let node = Node {
                    inner: vec![],
                    index: 0,
                    finalized: false,
                    subactions: vec![],
                    address,
                    data: classification.clone(),
                    trace_address: vec![],
                };

                let mut root = Root {
                    head:        node,
                    tx_hash:     trace.tx_hash,
                    private:     false,
                    gas_details: GasDetails {
                        coinbase_transfer:   None,
                        gas_used:            trace.gas_used,
                        effective_gas_price: trace.effective_price,
                        priority_fee:        trace.effective_price
                            - header.base_fee_per_gas.unwrap(),
                    },
                };

                for (index, trace) in trace.trace.into_iter().enumerate() {
                    root.gas_details.coinbase_transfer =
                        self.get_coinbase_transfer(header.beneficiary, &trace.trace.action);

                    let from_addr = trace.get_from_addr();
                    let classification = self.classify_node(trace.clone(), (index + 1) as u64);

                    let node = Node {
                        index:         (index + 1) as u64,
                        inner:         vec![],
                        finalized:     false,
                        subactions:    vec![],
                        address:       from_addr,
                        data:          classification,
                        trace_address: trace.trace.trace_address,
                    };

                    root.insert(node);
                }

                Some(root)
            })
            .collect::<Vec<Root<Actions>>>();

        let mut tree = TimeTree {
            roots,
            header,
            eth_prices: metadata.eth_prices.clone(),
            avg_priority_fee: 0,
        };

        self.try_classify_unknown_exchanges(&mut tree);
        // self.try_classify_flashloans(&mut tree);

        // remove duplicate swaps
        // tree.remove_duplicate_data(
        //     |node| node.data.is_swap(),
        //     |other_nodes, node| {
        //         let Actions::Swap(swap_data) = &node.data else { unreachable!() };
        //         other_nodes
        //             .into_iter()
        //             .filter_map(|(index, data)| {
        //                 let Actions::Transfer(transfer) = data else { return None };
        //                 if transfer.amount == swap_data.amount_in
        //                     && transfer.token == swap_data.token_in
        //                 {
        //                     return Some(*index)
        //                 }
        //                 None
        //             })
        //             .collect::<Vec<_>>()
        //     },
        //     |node| (node.index, node.data.clone()),
        // );
        // // remove duplicate mints
        // tree.remove_duplicate_data(
        //     |node| node.data.is_mint(),
        //     |other_nodes, node| {
        //         let Actions::Mint(mint_data) = &node.data else { unreachable!() };
        //         other_nodes
        //             .into_iter()
        //             .filter_map(|(index, data)| {
        //                 let Actions::Transfer(transfer) = data else { return None };
        //                 for (amount, token) in
        // mint_data.amount.iter().zip(&mint_data.token) {
        // if transfer.amount.eq(amount) && transfer.token.eq(token) {
        //                         return Some(*index)
        //                     }
        //                 }
        //                 None
        //             })
        //             .collect::<Vec<_>>()
        //     },
        //     |node| (node.index, node.data.clone()),
        // );

        tree.finalize_tree();

        tree
    }

    fn get_coinbase_transfer(&self, builder: Address, action: &Action) -> Option<u128> {
        match action {
            Action::Call(action) => {
                if action.to == builder {
                    return Some(action.value.to())
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn classify_node(&self, trace: TransactionTraceWithLogs, index: u64) -> Actions {
        let from_address = trace.get_from_addr();
        let target_address = trace.get_to_address();

        if let Some(protocol) = PROTOCOL_ADDRESS_MAPPING.get(&target_address.0) {
            if let Some(classifier) = &protocol.0 {
                let calldata = trace.get_calldata();
                let return_bytes = trace.get_return_calldata();
                let sig = &calldata[0..4];
                let res = protocol.1.try_decode(&calldata).unwrap();
                let d = classifier.dispatch(
                    sig,
                    index,
                    res,
                    return_bytes.clone(),
                    from_address,
                    target_address,
                    &trace.logs,
                );

                if let Some(res) = d {
                    return res
                }
            }
        }

        let rem = trace
            .logs
            .iter()
            .filter(|log| log.topics[0] == TRANSFER_TOPIC)
            .cloned()
            .collect::<Vec<Log>>();

        if rem.len() == 1 {
            if let Some((addr, from, to, value)) = self.decode_transfer(&rem[0]) {
                return Actions::Transfer(NormalizedTransfer {
                    index,
                    to,
                    from,
                    token: addr,
                    amount: value,
                })
            }
        }

        Actions::Unclassified(trace)
    }

    /// tries to prove dyn mint, dyn burn and dyn swap.
    pub(crate) fn prove_dyn_action(
        &self,
        node: &mut Node<Actions>,
        token_0: Address,
        token_1: Address,
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
            let (t1, from1, to1, value1) = transfer_data.remove(0);

            // sending 2 transfers to same addr
            if to0 == to1 && from0 == from1 {
                // burn
                if to0 == node.address {
                    return Some(Actions::Burn(NormalizedBurn {
                        to:        to0,
                        recipient: to1,
                        index:     node.index,
                        from:      from0,
                        token:     vec![t0, t1],
                        amount:    vec![value0, value1],
                    }))
                }
                // mint
                else {
                    return Some(Actions::Mint(NormalizedMint {
                        from:      to0,
                        recipient: to1,
                        index:     node.index,
                        to:        to0,
                        token:     vec![t0, t1],
                        amount:    vec![value0, value1],
                    }))
                }
            }
            // if to0 is to our addr then its the out token
            if to0 == addr {
                return Some(Actions::Swap(NormalizedSwap {
                    index:      node.index,
                    from:       to1,
                    pool:       to0,
                    token_in:   t1,
                    token_out:  t0,
                    amount_in:  value1,
                    amount_out: value0,
                }))
            } else {
                return Some(Actions::Swap(NormalizedSwap {
                    index:      node.index,
                    from:       to0,
                    pool:       to1,
                    token_in:   t0,
                    token_out:  t1,
                    amount_in:  value0,
                    amount_out: value1,
                }))
            }
        }
        // pure mint and burn
        if transfer_data.len() == 1 {
            let (token, from, to, value) = transfer_data.remove(0);
            if from == addr {
                return Some(Actions::Mint(NormalizedMint {
                    from,
                    recipient: to,
                    index: node.index,
                    to,
                    token: vec![token],
                    amount: vec![value],
                }))
            } else {
                return Some(Actions::Burn(NormalizedBurn {
                    to,
                    recipient: to,
                    index: node.index,
                    from,
                    token: vec![token],
                    amount: vec![value],
                }))
            }
        }

        None
    }

    fn decode_transfer(&self, log: &Log) -> Option<(Address, Address, Address, U256)> {
        if log.topics.get(0) == Some(&TRANSFER_TOPIC.into()) {
            let from = Address::from_slice(&log.topics[1][12..]);
            let to = Address::from_slice(&log.topics[2][12..]);
            let data = U256::try_from_be_slice(&log.data[..]).unwrap();
            return Some((log.address, from, to, data))
        }

        None
    }

    /// checks to see if we have a direct to <> from mapping for underlying
    /// transfers
    pub(crate) fn is_possible_exchange(&self, actions: Vec<Actions>) -> bool {
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
    pub(crate) fn try_classify_exchange(
        &self,
        node: &mut Node<Actions>,
    ) -> Option<(Address, (Address, Address), Actions)> {
        let addr = node.address;
        let subactions = node.get_all_sub_actions();

        let mut transfers = subactions
            .iter()
            .flat_map(|i| if let Actions::Transfer(t) = i { Some(t) } else { None })
            .map(|data| (data.token, data.from, data.to, data.amount, data.index))
            .combinations(2)
            .collect::<Vec<_>>();

        if transfers.len() < 2 {
            return None
        }

        transfers
            .into_par_iter()
            .filter_map(|mut transfer| {
                let (t0, from0, to0, value0, index0) = transfer.remove(0);
                let (t1, from1, to1, value1, index1) = transfer.remove(0);

                // diff tokens, direct from to mappings
                if t0 != t1 && (from0 == to1 && from1 == to0) {
                    // if the first swap occurred after the second
                    let swap = if index0 > index1 {
                        Actions::Swap(NormalizedSwap {
                            pool:       to1,
                            index:      node.index,
                            from:       from1,
                            token_in:   t1,
                            token_out:  t0,
                            amount_in:  value1,
                            amount_out: value0,
                        })
                    } else {
                        Actions::Swap(NormalizedSwap {
                            pool:       to0,
                            index:      node.index,
                            from:       from0,
                            token_in:   t0,
                            token_out:  t1,
                            amount_in:  value0,
                            amount_out: value1,
                        })
                    };
                    return Some((addr, (t0, t1), swap))
                }
                None
            })
            .min_by(|x,y| x.2.get_index().cmp(y.2.get_index()))
    }

    // fn dyn_flashloan_classify(&self, tree: &mut TimeTree<Actions>) {
    //     tree.remove_duplicate_data(find, classify, info)
    // }

    pub(crate) fn try_classify_unknown_exchanges(&self, tree: &mut TimeTree<Actions>) {
        // Acquire the read lock once
        let known_dyn_protocols_read = self.known_dyn_protocols.read();

        let new_classifed_exchanges = tree.dyn_classify(
            |address, node| {
                // we can dyn classify this shit
                if PROTOCOL_ADDRESS_MAPPING.contains_key(&address.0) {
                    // this is already classified
                    return (false, false)
                }
                if known_dyn_protocols_read.contains_key(&address)
                    || self.is_possible_exchange(node.get_all_sub_actions())
                {
                    if node.data.is_transfer() {
                        println!("transfer trigger: {:?}", node.data);
                        return (true, true)
                    } else {
                        return (true, false)
                    }
                }

                (false, false)
            },
            |node| {
                if known_dyn_protocols_read.contains_key(&node.address) {
                    let (token_0, token_1) = known_dyn_protocols_read.get(&node.address).unwrap();
                    if let Some(res) = self.prove_dyn_action(node, *token_0, *token_1) {
                        node.data = res;
                    }
                } else if let Some((ex_addr, tokens, action)) = self.try_classify_exchange(node) {
                    node.data = action;

                    return Some((ex_addr, tokens))
                }
                None
            },
        );
        // Drop the read lock
        drop(known_dyn_protocols_read);

        if !new_classifed_exchanges.is_empty() {
            let mut known_dyn_protocols_write = self.known_dyn_protocols.write();
            new_classifed_exchanges.into_iter().for_each(|(k, v)| {
                known_dyn_protocols_write.insert(k, v);
            });
        };
    }

    /// in order to classify flashloans, we need to check for couple things
    /// 1) call to address that does a callback.
    /// 2) callback address receives funds
    /// 3) when this callscope exits, there is a transfer of the value or more
    /// to the inital call address
    fn try_classify_flashloans(&self, tree: &mut TimeTree<Actions>) {
        // lets check and grab all instances such that there is a transfer of a
        // token from and to the same address where the to transfer has
        // equal or more value
        // tree.inspect_all(|node| {
        //     let mut transfers = HashMap::new();
        //
        //     node.get_all_sub_actions().into_iter().for_each(|action| {
        //         if let Actions::Transfer(t) = action {
        //             match transfers.entry(t.token) {
        //                 Entry::Vacant(v) => {
        //                     v.insert(vec![(t.to, t.from, t.amount)]);
        //                 }
        //                 Entry::Occupied(mut o) => {
        //                     o.get_mut().push((t.to, t.from, t.amount));
        //                 }
        //             }
        //         }
        //     });
        //
        //     // checks for same address transfer and also verifies that mor
        //     let has_proper_payment_scheme = transfers
        //         .values()
        //         .into_iter()
        //         .filter_map(|v| {
        //             let (to, from, amount) = v.into_iter().multiunzip();
        //             // this is so bad but so tired and wanna get this done.
        // def need to fix             for i in 0..to.len() {
        //                 for j in 0..to.len() {
        //                     if i == j {
        //                         continue
        //                     }
        //
        //                     // we check both directions to minimize loops
        //                     if to[i] == from[j]
        //                         && to[j] == from[i]
        //                         && (i > j && amount[i] >= amount[j])
        //                         || (i < j && amount[i] <= amount[j])
        //                     {
        //                         return Some((to, from))
        //                     }
        //                 }
        //             }
        //             None
        //         })
        //         .collect::<Vec<_>>();
        //
        //     if has_proper_payment_scheme.is_empty() {
        //         return false
        //     }
        //
        //     // if we don't have this shit then we can quick return and do
        // less calcs     if !has_proper_payment_scheme.iter().any(|(to,
        // from)| {         let sub = node.all_sub_addresses();
        //         sub.contains(to) && sub.contains(from)
        //     }) {
        //         return false
        //     }
        //
        //     // lets make sure that we have the underlying to and from
        // addresses in our     // subtree, if not, we can early return
        // and avoid beefy calc
        //
        //     // lets now verify this sandwich property
        //     has_proper_payment_scheme.into_iter().any(|(to, from)| {
        //         // inspect lower to see if we get this based shit_
        //         let mut _t = Vec::new();
        //         node.inspect(&mut _t, &|node| {
        //             if node.address == to {
        //                 // node.
        //             }
        //         })
        //     });
        //
        //     let paths = node
        //         .tree_right_path()
        //         .windows(3)
        //         .any(|[addr0, addr1, addr2]| {});
        //
        //     //
        //
        //     false
        // });
    }
}

#[cfg(test)]
pub mod test {
    use std::collections::HashSet;

    use brontes_classifier::test_utils::build_raw_test_tree;
    use brontes_core::{decoding::parser::TraceParser, test_utils::init_trace_parser};
    use brontes_database::database::Database;
    use brontes_types::{normalized_actions::Actions, test_utils::force_call_action, tree::Node};
    use reth_primitives::Address;
    use reth_rpc_types::trace::parity::{TraceType, TransactionTrace};
    use reth_tracing::TracingClient;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::test_utils::get_traces_with_meta;

    #[tokio::test]
    #[serial]
    async fn test_dyn_classifier() {
        let block_num = 18530326;
        dotenv::dotenv().ok();

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();

        let classifier = Classifier::new();

        let (traces, header, metadata) = get_traces_with_meta(&tracer, &db, block_num).await;

        let mut tree = classifier.build_tree(traces, header, &metadata);
        let root = tree.roots.remove(30);

        let swaps = root.collect(&|node| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.get_all_sub_actions()
                    .iter()
                    .any(|s| s.is_swap() || s.is_transfer()),
            )
        });

        println!("{:#?}", swaps);
    }
}
