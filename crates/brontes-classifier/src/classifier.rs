use std::sync::Arc;

use alloy_primitives::Log;
use alloy_sol_types::SolEvent;
use brontes_database_libmdbx::{
    tables::AddressToProtocol, types::address_to_protocol::StaticBindingsDb, AddressToFactory,
    Libmdbx,
};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    extra_processing::ExtraProcessing,
    normalized_actions::{Actions, NormalizedAction, NormalizedSwapWithFee, NormalizedTransfer},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use futures::future::join_all;
use hex_literal::hex;
use itertools::Itertools;
use reth_db::transaction::DbTx;
use reth_primitives::{alloy_primitives::FixedBytes, Address, Header, B256, U256};
use reth_rpc_types::trace::parity::Action;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};

use crate::{classifiers::*, ActionCollection, FactoryDecoderDispatch, StaticBindings};

const TRANSFER_TOPIC: B256 =
    FixedBytes(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

/// goes through and classifies all exchanges as-well as missing data
#[derive(Debug, Clone)]
pub struct Classifier<'db, T: TracingProvider> {
    libmdbx:  &'db Libmdbx,
    provider: Arc<T>,
    sender:   UnboundedSender<DexPriceMsg>,
}

impl<'db, T: TracingProvider> Classifier<'db, T> {
    pub fn new(
        libmdbx: &'db Libmdbx,
        sender: UnboundedSender<DexPriceMsg>,
        provider: Arc<T>,
    ) -> Self {
        Self { libmdbx, sender, provider }
    }

    pub fn close(&self) {
        self.sender.send(DexPriceMsg::Closed).unwrap();
    }

    pub async fn build_block_tree(
        &self,
        traces: Vec<TxTrace>,
        header: Header,
    ) -> (ExtraProcessing, BlockTree<Actions>) {
        // TODO: this needs to be cleaned up this is so ugly
        let (missing_data_requests, pool_updates, further_classification_requests, tx_roots): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = join_all(
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_idx, mut trace)| async move {
                    if trace.trace.is_empty() || !trace.is_success {
                        return None
                    }
                    let tx_hash = trace.tx_hash;

                    // post classification processing collectors
                    let mut missing_decimals = Vec::new();
                    let mut further_classification_requests = Vec::new();
                    let mut pool_updates: Vec<DexPriceMsg> = Vec::new();

                    let root_trace = trace.trace.remove(0);
                    let address = root_trace.get_from_addr();
                    let classification = self
                        .process_classification(
                            header.number,
                            tx_idx as u64,
                            0,
                            tx_hash,
                            root_trace,
                            &mut missing_decimals,
                            &mut further_classification_requests,
                            &mut pool_updates,
                        )
                        .await;

                    let node = Node::new(0, address, classification, vec![]);

                    let mut tx_root = Root {
                        position:    tx_idx,
                        head:        node,
                        tx_hash:     trace.tx_hash,
                        private:     false,
                        gas_details: GasDetails {
                            coinbase_transfer:   None,
                            gas_used:            trace.gas_used,
                            effective_gas_price: trace.effective_price,
                            priority_fee:        trace.effective_price
                                - (header.base_fee_per_gas.unwrap() as u128),
                        },
                    };

                    for (index, trace) in trace.trace.into_iter().enumerate() {
                        if let Some(coinbase) = &mut tx_root.gas_details.coinbase_transfer {
                            *coinbase += self
                                .get_coinbase_transfer(header.beneficiary, &trace.trace.action)
                                .unwrap_or_default()
                        } else {
                            tx_root.gas_details.coinbase_transfer =
                                self.get_coinbase_transfer(header.beneficiary, &trace.trace.action);
                        }

                        let classification = self
                            .process_classification(
                                header.number,
                                tx_idx as u64,
                                (index + 1) as u64,
                                tx_hash,
                                trace.clone(),
                                &mut missing_decimals,
                                &mut further_classification_requests,
                                &mut pool_updates,
                            )
                            .await;

                        let from_addr = trace.get_from_addr();

                        let node = Node::new(
                            (index + 1) as u64,
                            from_addr,
                            classification,
                            trace.trace.trace_address,
                        );

                        tx_root.insert(node);
                    }

                    // Here we reverse the requests to ensure that we always classify the most
                    // nested action & its children first. This is to prevent the
                    // case where we classify a parent action where its children also require
                    // further classification
                    let tx_classification_requests = if !further_classification_requests.is_empty()
                    {
                        further_classification_requests.reverse();
                        Some((tx_idx, further_classification_requests))
                    } else {
                        None
                    };

                    Some((missing_decimals, pool_updates, tx_classification_requests, tx_root))
                }),
        )
        .await
        .into_iter()
        .filter_map(|f| f)
        .multiunzip();

        // send out all updates
        pool_updates
            .into_iter()
            .flatten()
            .for_each(|update| self.sender.send(update).unwrap());

        let mut tree =
            BlockTree { tx_roots, header, eth_price: Default::default(), avg_priority_fee: 0 };

        self.deal_with_tax_tokens(&mut tree);
        self.remove_swap_transfers(&mut tree);
        self.remove_mint_transfers(&mut tree);
        self.remove_collect_transfers(&mut tree);
        self.finish_classification(&mut tree, further_classification_requests);

        tree.finalize_tree();

        let mut dec = missing_data_requests
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        // need to sort before we can dedup
        dec.sort();

        dec.dedup();

        let processing = ExtraProcessing { tokens_decimal_fill: dec };

        (processing, tree)
    }

    async fn process_classification(
        &self,
        block_number: u64,
        tx_index: u64,
        trace_index: u64,
        tx_hash: B256,
        trace: TransactionTraceWithLogs,
        missing_decimals: &mut Vec<Address>,
        further_classification_requests: &mut Vec<u64>,
        pool_updates: &mut Vec<DexPriceMsg>,
    ) -> Actions {
        let (update, classification) = self
            .classify_node(block_number, tx_index as u64, trace, trace_index, tx_hash)
            .await;

        // Here we are marking more complex actions that require data
        // that can only be retrieved by classifying it's action and
        // all subsequent child actions.
        if classification.continue_classification() {
            further_classification_requests.push(classification.get_trace_index());
        }

        if let Actions::Transfer(transfer) = &classification {
            if self.libmdbx.try_get_decimals(transfer.token).is_none() {
                missing_decimals.push(transfer.token);
            }
        }

        // if we have a discovered pool, check if its new
        update.into_iter().for_each(|update| {
            match update {
                DexPriceMsg::DiscoveredPool(pool, block) => {
                    if !self.libmdbx.contains_pool(pool.pool_address) {
                        self.sender
                            .send(DexPriceMsg::DiscoveredPool(pool.clone(), block))
                            .unwrap();

                        if self
                            .libmdbx
                            .insert_pool(
                                block_number,
                                pool.pool_address,
                                [pool.tokens[0], pool.tokens[1]],
                                pool.protocol,
                            )
                            .is_err()
                        {
                            error!("failed to insert discovered pool into libmdbx");
                        }
                    }
                }
                rest => {
                    pool_updates.push(rest);
                }
            };
        });

        classification
    }

    /// When a tax token takes a fee, They will swap from there token to a more
    /// stable token like eth before taking the fee. However this causes us
    /// problems as we will register this fee swap as part of the mev
    /// messing up our calculations
    fn deal_with_tax_tokens(&self, tree: &mut BlockTree<Actions>) {
        // remove swaps that originate from a transfer. This event only occurs
        // when a tax token is transfered and the taxed amount is swapped into
        // a more stable currency
        tree.modify_node_if_contains_childs(
            |node| {
                let mut has_transfer = false;
                let mut has_swap = false;

                for action in &node.get_all_sub_actions() {
                    if action.is_transfer() {
                        has_transfer = true;
                    } else if action.is_swap() {
                        has_swap = true;
                    }
                }
                if node.data.is_transfer() {
                    println!("{node:#?}");
                }
                (node.data.is_transfer(), has_swap && has_transfer)
            },
            |node| {
                info!("removing transfer with swap");
                let mut swap_idx = Vec::new();
                node.collect(
                    &mut swap_idx,
                    &|node| {
                        (node.data.is_swap(), node.subactions.iter().any(|action| action.is_swap()))
                    },
                    &|node| node.index,
                );

                swap_idx.into_iter().for_each(|idx| {
                    node.remove_node_and_children(idx);
                })
            },
        );

        // adjusts the amount in of the swap and notes the fee on the normalized type.
        // This is needed when swapping into the tax token as the amount out of the swap
        // will be wrong
        tree.modify_node_if_contains_childs(
            |node| {
                let mut has_transfer = false;
                let mut has_swap = false;
                for action in &node.get_all_sub_actions() {
                    if action.is_transfer() {
                        has_transfer = true;
                    } else if action.is_swap() {
                        has_swap = true;
                    }
                }
                (node.data.is_swap(), has_swap && has_transfer)
            },
            |node| {
                // collect all sub transfers
                let mut transfers = Vec::new();
                node.collect(
                    &mut transfers,
                    &|node| {
                        (
                            node.data.is_transfer(),
                            node.get_all_sub_actions()
                                .iter()
                                .any(|node| node.is_transfer()),
                        )
                    },
                    &|node| node.data.clone(),
                );

                transfers
                    .into_iter()
                    .filter_map(
                        |transfer| {
                            if let Actions::Transfer(t) = transfer {
                                Some(t)
                            } else {
                                None
                            }
                        },
                    )
                    .find_map(|transfer| {
                        let mut swap = node.data.clone().force_swap();
                        // if the swap matches the transfer but the transfer is a less amount of
                        // tokens than the swap says
                        if swap.token_out == transfer.token
                            && swap.pool == transfer.from
                            && swap.recipient == transfer.token
                            && swap.amount_out > transfer.amount
                        {
                            let fee_amount = swap.amount_out - transfer.amount;
                            swap.amount_out = transfer.amount;

                            let swap = Actions::SwapWithFee(NormalizedSwapWithFee {
                                swap,
                                fee_amount,
                                fee_token: transfer.token,
                            });
                            node.data = swap;

                            return Some(())
                        }

                        None
                    });
            },
        )
    }

    fn remove_swap_transfers(&self, tree: &mut BlockTree<Actions>) {
        tree.remove_duplicate_data(
            |node| {
                (
                    node.data.is_swap(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_swap()),
                )
            },
            |node| {
                (
                    node.data.is_transfer(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_transfer()),
                )
            },
            |node| (node.index, node.data.clone()),
            |other_nodes, node| {
                let Actions::Swap(swap_data) = &node.data else { unreachable!() };
                other_nodes
                    .into_iter()
                    .filter_map(|(index, data)| {
                        let Actions::Transfer(transfer) = data else { return None };
                        if (transfer.amount == swap_data.amount_in
                            || transfer.amount == swap_data.amount_out)
                            && (transfer.to == swap_data.pool || transfer.from == swap_data.pool)
                        {
                            return Some(*index)
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            },
        );
    }

    // need this for dyn classifying
    fn remove_mint_transfers(&self, tree: &mut BlockTree<Actions>) {
        tree.remove_duplicate_data(
            |node| {
                (
                    node.data.is_mint(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_mint()),
                )
            },
            |node| {
                (
                    node.data.is_transfer(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_transfer()),
                )
            },
            |node| (node.index, node.data.clone()),
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
        );
    }

    fn remove_collect_transfers(&self, tree: &mut BlockTree<Actions>) {
        tree.remove_duplicate_data(
            |node| {
                (
                    node.data.is_collect(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_collect()),
                )
            },
            |node| {
                (
                    node.data.is_transfer(),
                    node.get_all_sub_actions()
                        .into_iter()
                        .any(|data| data.is_transfer()),
                )
            },
            |node| (node.index, node.data.clone()),
            |other_nodes, node| {
                let Actions::Collect(collect_data) = &node.data else { unreachable!() };
                other_nodes
                    .into_iter()
                    .filter_map(|(index, data)| {
                        let Actions::Transfer(transfer) = data else { return None };
                        for (amount, token) in collect_data.amount.iter().zip(&collect_data.token) {
                            if transfer.amount.eq(amount) && transfer.token.eq(token) {
                                return Some(*index)
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            },
        );
    }

    fn get_coinbase_transfer(&self, builder: Address, action: &Action) -> Option<u128> {
        match action {
            Action::Call(action) => {
                if action.to == builder && !action.value.is_zero() {
                    return Some(action.value.to())
                }
                None
            }
            _ => None,
        }
    }

    async fn classify_node(
        &self,
        block: u64,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
        tx_hash: B256,
    ) -> (Vec<DexPriceMsg>, Actions) {
        // we don't classify static calls
        if trace.is_static_call() {
            return (vec![], Actions::Unclassified(trace))
        }
        if trace.trace.error.is_some() {
            return (vec![], Actions::Revert)
        }

        let from_address = trace.get_from_addr();
        let target_address = trace.get_to_address();

        //TODO: get rid of these unwraps
        let db_tx = self.libmdbx.ro_tx().unwrap();

        if let Some(protocol) = db_tx.get::<AddressToProtocol>(target_address).unwrap() {
            let classifier: Box<dyn ActionCollection> = match protocol {
                StaticBindingsDb::UniswapV2 => Box::new(UniswapV2Classifier::default()),
                StaticBindingsDb::SushiSwapV2 => Box::new(SushiSwapV2Classifier::default()),
                StaticBindingsDb::UniswapV3 => Box::new(UniswapV3Classifier::default()),
                StaticBindingsDb::SushiSwapV3 => Box::new(SushiSwapV3Classifier::default()),
                StaticBindingsDb::CurveCryptoSwap => Box::new(CurveCryptoSwapClassifier::default()),
                StaticBindingsDb::AaveV2 => Box::new(AaveV2Classifier::default()),
                StaticBindingsDb::AaveV3 => Box::new(AaveV3Classifier::default()),
                StaticBindingsDb::UniswapX => Box::new(UniswapXClassifier::default()),
            };

            let calldata = trace.get_calldata();
            let return_bytes = trace.get_return_calldata();
            let sig = &calldata[0..4];
            let res = Into::<StaticBindings>::into(protocol)
                .try_decode(&calldata)
                .map(|data| {
                    classifier.dispatch(
                        sig,
                        trace_index,
                        data,
                        return_bytes.clone(),
                        from_address,
                        target_address,
                        &trace.logs,
                        &db_tx,
                        block,
                        tx_idx,
                    )
                })
                .ok()
                .flatten();

            if let Some(res) = res {
                return (vec![DexPriceMsg::Update(res.0)], res.1)
            }
        }

        if let Some(protocol) = db_tx.get::<AddressToFactory>(target_address).unwrap() {
            let discovered_pools = match protocol {
                StaticBindingsDb::UniswapV2 | StaticBindingsDb::SushiSwapV2 => {
                    UniswapDecoder::dispatch(
                        crate::UniswapV2Factory::PairCreated::SIGNATURE_HASH.0,
                        self.provider.clone(),
                        protocol,
                        &trace.logs,
                        block,
                        tx_hash,
                    )
                    .await
                }
                StaticBindingsDb::UniswapV3 | StaticBindingsDb::SushiSwapV3 => {
                    UniswapDecoder::dispatch(
                        crate::UniswapV3Factory::PoolCreated::SIGNATURE_HASH.0,
                        self.provider.clone(),
                        protocol,
                        &trace.logs,
                        block,
                        tx_hash,
                    )
                    .await
                }
                _ => {
                    vec![]
                }
            }
            .into_iter()
            .map(|data| DexPriceMsg::DiscoveredPool(data, block))
            .collect_vec();

            // TODO: do we want to make a normalized pool deploy?
            return (discovered_pools, Actions::Unclassified(trace))
        }

        if trace.logs.len() > 0 {
            // A transfer should always be in its own call trace and have 1 log.  
            // if forever reason there is a case with multiple logs, we take the first
            // transfer
            for log in &trace.logs {
                if let Some((addr, from, to, value)) = self.decode_transfer(log) {
                    return (
                        vec![],
                        Actions::Transfer(NormalizedTransfer {
                            trace_index,
                            to,
                            from,
                            token: addr,
                            amount: value,
                        }),
                    )
                }
            }
        }

        (vec![], Actions::Unclassified(trace))
    }

    /// This function is used to finalize the classification of complex actions
    /// that contain nested sub-actions that are required to finalize the higher
    /// level classification (e.g: flashloan actions)
    fn finish_classification(
        &self,
        tree: &mut BlockTree<Actions>,
        further_classification_requests: Vec<Option<(usize, Vec<u64>)>>,
    ) {
        tree.collect_and_classify(&further_classification_requests)
    }

    fn decode_transfer(&self, log: &Log) -> Option<(Address, Address, Address, U256)> {
        if log.topics().len() != 3 {
            return None
        }

        if log.topics().get(0) == Some(&TRANSFER_TOPIC) {
            let from = Address::from_slice(&log.topics()[1][12..]);
            let to = Address::from_slice(&log.topics()[2][12..]);
            let data = U256::try_from_be_slice(&log.data.data[..]).unwrap();
            return Some((log.address, from, to, data))
        }

        None
    }
}

#[cfg(test)]
pub mod test {
    use std::{
        collections::{HashMap, HashSet},
        env,
    };

    use brontes_classifier::test_utils::build_raw_test_tree;
    use brontes_core::{
        decoding::{parser::TraceParser, TracingProvider},
        test_utils::init_trace_parser,
    };
    use brontes_database::{clickhouse::Clickhouse, Metadata};
    use brontes_database_libmdbx::{types::address_to_protocol::StaticBindingsDb, Libmdbx};
    use brontes_types::{
        normalized_actions::Actions,
        structured_trace::TxTrace,
        test_utils::force_call_action,
        tree::{BlockTree, Node},
    };
    use reth_primitives::{Address, Header};
    use reth_rpc_types::trace::parity::{TraceType, TransactionTrace};
    use reth_tracing_ext::TracingClient;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::Classifier;

    #[tokio::test]
    #[serial]
    async fn test_remove_swap_transfer() {
        let block_num = 18530326;
        dotenv::dotenv().ok();
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();
        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx, &libmdbx, 6);
        let db = Clickhouse::default();

        let tree = build_raw_test_tree(&tracer, &db, &libmdbx, block_num).await;
        let jarad = tree.roots[1].tx_hash;

        let swap = tree.collect(jarad, |node| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        });
        println!("{:#?}", swap);
        let mut swaps: HashMap<Address, HashSet<U256>> = HashMap::default();

        for i in &swap {
            if let Actions::Swap(s) = i {
                swaps.entry(s.token_in).or_default().insert(s.amount_in);
                swaps.entry(s.token_out).or_default().insert(s.amount_out);
            }
        }

        for i in &swap {
            if let Actions::Transfer(t) = i {
                if swaps.get(&t.token).map(|i| i.contains(&t.amount)) == Some(true) {
                    assert!(false, "found a transfer that was part of a swap");
                }
            }
        }
    }
}
