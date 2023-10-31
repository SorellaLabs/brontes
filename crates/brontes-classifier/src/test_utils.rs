use std::collections::{HashMap, HashSet};

use brontes_core::decoding::parser::TraceParser;
use brontes_database::{database::Database, Metadata};
use brontes_types::{
    normalized_actions::{
        Actions, NormalizedBurn, NormalizedMint, NormalizedSwap, NormalizedTransfer,
    },
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    tree::{GasDetails, Node, Root, TimeTree},
};
use hex_literal::hex;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, Header, H256, U256};
use reth_rpc_types::{trace::parity::Action, Log};
use reth_tracing::TracingClient;

use crate::{Classifier, StaticReturnBindings, PROTOCOL_ADDRESS_MAPPING};

const TRANSFER_TOPIC: H256 =
    H256(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

pub async fn build_raw_test_tree(
    tracer: &TraceParser<TracingClient>,
    db: Database,
    block_number: u64,
) -> TimeTree<Actions> {
    let (traces, header, metadata) = get_traces_with_meta(tracer, db, block_number).await;
    let roots = traces
        .into_par_iter()
        .filter_map(|mut trace| {
            if trace.trace.is_empty() {
                return None
            }

            let root_trace = trace.trace[0].clone();
            let address = root_trace.get_from_addr();
            let classification = classify_node(trace.trace.remove(0), 0);

            let node = Node {
                inner: vec![],
                index: 0,
                finalized: !classification.is_unclassified(),
                subactions: vec![],
                address,
                data: classification,
                trace_address: root_trace.trace.trace_address,
            };

            let mut root = Root {
                head:        node,
                tx_hash:     trace.tx_hash,
                private:     false,
                gas_details: GasDetails {
                    coinbase_transfer:   None,
                    gas_used:            trace.gas_used,
                    effective_gas_price: trace.effective_price,
                    priority_fee:        trace.effective_price - header.base_fee_per_gas.unwrap(),
                },
            };

            for (index, trace) in trace.trace.into_iter().enumerate() {
                root.gas_details.coinbase_transfer =
                    get_coinbase_transfer(header.beneficiary, &trace.trace.action);

                let from_addr = trace.get_from_addr();
                let classification = classify_node(trace.clone(), (index + 1) as u64);
                let node = Node {
                    index:         (index + 1) as u64,
                    inner:         vec![],
                    finalized:     !classification.is_unclassified(),
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

    TimeTree { roots, header, eth_prices: metadata.eth_prices.clone(), avg_priority_fee: 0 }
}

fn classify_node(trace: TransactionTraceWithLogs, index: u64) -> Actions {
    let from_address = trace.get_from_addr();
    let target_address = trace.get_to_address();

    if let Some(protocol) = PROTOCOL_ADDRESS_MAPPING.get(&target_address.0) {
        if let Some(classifier) = &protocol.0 {
            let calldata = trace.get_calldata();
            let return_bytes = trace.get_return_calldata();
            let sig = &calldata[0..4];
            let res: StaticReturnBindings = protocol.1.try_decode(&calldata).unwrap();

            if let Some(res) = classifier.dispatch(
                sig,
                index,
                res,
                return_bytes,
                from_address,
                target_address,
                &trace.logs,
            ) {
                return res
            }
        }
    }

    let rem = trace
        .logs
        .iter()
        .filter(|log| log.address == from_address)
        .cloned()
        .collect::<Vec<Log>>();

    if rem.len() == 1 {
        if let Some((addr, from, to, value)) = helper_decode_transfer(&rem[0]) {
            return Actions::Transfer(NormalizedTransfer {
                index,
                to,
                from,
                token: addr,
                amount: value,
            })
        }
    }

    Actions::Unclassified(trace, rem)
}

pub fn helper_decode_transfer(log: &Log) -> Option<(Address, Address, Address, U256)> {
    if log.topics.get(0) == Some(&TRANSFER_TOPIC.into()) {
        let from = Address::from_slice(&log.topics[1][..20]);
        let to = Address::from_slice(&log.topics[2][..20]);
        let data = U256::try_from_be_slice(&log.data[..]).unwrap();
        return Some((log.address, from, to, data))
    }

    None
}

pub async fn get_traces_with_meta(
    tracer: &TraceParser<TracingClient>,
    db: Database,
    block_number: u64,
) -> (Vec<TxTrace>, Header, Metadata) {
    let (traces, header) = tracer.execute_block(block_number).await.unwrap();
    let metadata = db.get_metadata(block_number).await;
    (traces, header, metadata)
}

fn get_coinbase_transfer(builder: Address, action: &Action) -> Option<u64> {
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

pub fn helper_prove_dyn_action(
    classifier: &Classifier,
    node: &mut Node<Actions>,
    token_0: Address,
    token_1: Address,
) -> Option<Actions> {
    classifier.prove_dyn_action(node, token_0, token_1)
}

pub fn helper_try_classify_unknown_exchanges(
    classifier: &Classifier,
    tree: &mut TimeTree<Actions>,
) {
    classifier.try_classify_unknown_exchanges(tree)
}

pub fn helper_try_classify_unknown_exchanges2(classifier: &Classifier, tree: TimeTree<Actions>) {
    let known_dyn_protocols_read = classifier.known_dyn_protocols.read();

    let mut root = &mut tree.roots.first().unwrap();
    root.dyn_classify(
        &|address, sub_actions| {
            // we can dyn classify this shit
            if PROTOCOL_ADDRESS_MAPPING.contains_key(&address.0) {
                // this is already classified
                return false
            }
            if known_dyn_protocols_read.contains_key(&address)
                || classifier.is_possible_exchange(sub_actions)
            {
                return true
            }

            false
        },
        &|node| {
            if known_dyn_protocols_read.contains_key(&node.address) {
                let (token_0, token_1) = known_dyn_protocols_read.get(&node.address).unwrap();
                if let Some(res) = classifier.prove_dyn_action(node, *token_0, *token_1) {
                    // we have reduced the lower part of the tree. we can delete this now
                    node.inner.clear();
                    node.data = res;
                }
            } else if let Some((ex_addr, tokens, action)) = classifier.try_clasify_exchange(node) {
                node.inner.clear();
                node.data = action;

                return Some((ex_addr, tokens))
            }
            None
        },
    );
}
