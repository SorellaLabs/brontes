use std::{cmp::min, sync::Arc};

use alloy_primitives::{Log, U256};
use brontes_core::missing_token_info::load_missing_token_info;
use brontes_types::{
    normalized_actions::{
        pool::NormalizedNewPool, MultiCallFrameClassification, MultiFrameRequest, NormalizedAction,
        NormalizedEthTransfer, NormalizedTransfer,
    },
    tree::root::NodeData,
    ToScaledRational,
};
use malachite::{num::basic::traits::Zero, Rational};
mod tree_pruning;
mod utils;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::{Actions, SelfdestructWithIndex},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use futures::future::join_all;
use itertools::Itertools;
use malachite::num::arithmetic::traits::Abs;
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::{Action, CallType};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};
use tree_pruning::account_for_tax_tokens;
use utils::{decode_transfer, get_coinbase_transfer};

use self::erc20::try_decode_transfer;
use crate::{
    classifiers::*, multi_frame_classification::parse_multi_frame_requests, ActionCollection,
    FactoryDiscoveryDispatch,
};

//TODO: Document this module
#[derive(Debug, Clone)]
pub struct Classifier<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx:               &'db DB,
    provider:              Arc<T>,
    pricing_update_sender: UnboundedSender<DexPriceMsg>,
}

impl<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> Classifier<'db, T, DB> {
    pub fn new(
        libmdbx: &'db DB,
        pricing_update_sender: UnboundedSender<DexPriceMsg>,
        provider: Arc<T>,
    ) -> Self {
        Self { libmdbx, pricing_update_sender, provider }
    }

    pub async fn build_block_tree(
        &self,
        traces: Vec<TxTrace>,
        header: Header,
        generate_pricing: bool,
    ) -> BlockTree<Actions> {
        if !generate_pricing {
            self.pricing_update_sender
                .send(DexPriceMsg::DisablePricingFor(header.number))
                .unwrap();
        }

        let tx_roots = self.build_tx_trees(traces, &header).await;
        let mut tree = BlockTree::new(header, tx_roots.len());

        // send out all updates
        let further_classification_requests = self.process_tx_roots(tx_roots, &mut tree);
        account_for_tax_tokens(&mut tree);

        self.finish_classification(&mut tree, further_classification_requests);
        tree.finalize_tree();

        tree
    }

    fn process_tx_roots(
        &self,
        tx_roots: Vec<TxTreeResult>,
        tree: &mut BlockTree<Actions>,
    ) -> Vec<Option<(usize, Vec<MultiCallFrameClassification<Actions>>)>> {
        tx_roots
            .into_iter()
            .map(|root_data| {
                tree.insert_root(root_data.root);
                root_data.pool_updates.into_iter().for_each(|update| {
                    tracing::debug!("sending update");
                    self.pricing_update_sender.send(update).unwrap();
                });

                root_data
                    .further_classification_requests
                    .map(|(tx, requests)| (tx, parse_multi_frame_requests(requests)))
            })
            .collect_vec()
    }

    pub(crate) async fn build_tx_trees(
        &self,
        traces: Vec<TxTrace>,
        header: &Header,
    ) -> Vec<TxTreeResult> {
        join_all(
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_idx, mut trace)| async move {
                    // here only traces where the root tx failed are filtered out
                    if trace.trace.is_empty() || !trace.is_success {
                        tracing::debug!(
                            empty = trace.trace.is_empty(),
                            is_success = trace.is_success
                        );
                        return None
                    }
                    // post classification processing collectors
                    let mut further_classification_requests = Vec::new();
                    let mut pool_updates: Vec<DexPriceMsg> = Vec::new();

                    let root_trace = trace.trace.remove(0);

                    let address = root_trace.get_from_addr();
                    let trace_idx = root_trace.trace_idx;

                    let classification = self
                        .process_classification(
                            header.number,
                            None,
                            &NodeData(vec![]),
                            tx_idx as u64,
                            trace_idx,
                            root_trace,
                            &trace.trace,
                            &mut further_classification_requests,
                            &mut pool_updates,
                        )
                        .await;

                    let node = Node::new(trace_idx, address, vec![]);

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
                                - (header.base_fee_per_gas.unwrap_or_default() as u128),
                        },
                        data_store:  NodeData(vec![Some(classification)]),
                    };

                    let tx_trace = &trace.trace;
                    for trace in &trace.trace {
                        let from_addr = trace.get_from_addr();

                        let node = Node::new(
                            trace.trace_idx,
                            from_addr,
                            trace.trace.trace_address.clone(),
                        );

                        if trace.trace.error.is_none() {
                            if let Some(coinbase_transfer) =
                                get_coinbase_transfer(header.beneficiary, &trace.trace.action)
                            {
                                if let Some(coinbase) = &mut tx_root.gas_details.coinbase_transfer {
                                    *coinbase += coinbase_transfer;
                                } else {
                                    tx_root.gas_details.coinbase_transfer = Some(coinbase_transfer);
                                }

                                let classification = Actions::EthTransfer(NormalizedEthTransfer {
                                    from:              from_addr,
                                    to:                trace.get_to_address(),
                                    value:             trace.get_msg_value(),
                                    trace_index:       trace.trace_idx,
                                    coinbase_transfer: true,
                                });

                                tx_root.insert(node, vec![classification]);
                                continue
                            }
                        }

                        let classification = self
                            .process_classification(
                                header.number,
                                Some(&tx_root.head),
                                &tx_root.data_store,
                                tx_idx as u64,
                                trace.trace_idx,
                                trace.clone(),
                                tx_trace,
                                &mut further_classification_requests,
                                &mut pool_updates,
                            )
                            .await;

                        tx_root.insert(node, classification);
                    }

                    // Here we reverse the requests to ensure that we always classify the most
                    // nested action & its children first. This is to prevent the
                    // case where we classify a parent action where its children also require
                    // further classification.
                    let tx_classification_requests = if !further_classification_requests.is_empty()
                    {
                        further_classification_requests.reverse();
                        Some((tx_idx, further_classification_requests))
                    } else {
                        None
                    };
                    Some(TxTreeResult {
                        root: tx_root,
                        further_classification_requests: tx_classification_requests,
                        pool_updates,
                    })
                }),
        )
        .await
        .into_iter()
        .flatten()
        .collect_vec()
    }

    async fn process_classification(
        &self,
        block_number: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Actions>,
        tx_index: u64,
        trace_index: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
        further_classification_requests: &mut Vec<MultiFrameRequest>,
        pool_updates: &mut Vec<DexPriceMsg>,
    ) -> Vec<Actions> {
        let (update, classification) = self
            .classify_node(
                block_number,
                root_head,
                node_data_store,
                tx_index,
                trace,
                full_trace,
                trace_index,
            )
            .await;

        // Here we are marking more complex actions that require data
        // that can only be retrieved by classifying it's action and
        // all subsequent child actions.
        further_classification_requests.extend(
            classification
                .iter()
                .filter_map(|action| action.multi_frame_classification()),
        );

        update.into_iter().for_each(|update| {
            match update {
                pool @ DexPriceMsg::DiscoveredPool(_) => {
                    self.pricing_update_sender.send(pool).unwrap();
                }
                rest => {
                    pool_updates.push(rest);
                }
            };
        });

        classification
    }

    fn contains_pool(&self, address: Address) -> bool {
        self.libmdbx.get_protocol(address).is_ok()
    }

    async fn classify_node(
        &self,
        block: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Actions>,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Vec<Actions>) {
        if trace.trace.error.is_some() {
            return (vec![], vec![Actions::Revert])
        }
        let (pricing, base_action) = match trace.action_type() {
            Action::Call(_) => {
                self.classify_call(block, tx_idx, trace.clone(), full_trace, trace_index)
                    .await
            }
            Action::Create(_) => {
                self.classify_create(
                    block,
                    root_head,
                    node_data_store,
                    tx_idx,
                    trace.clone(),
                    trace_index,
                )
                .await
            }
            Action::Selfdestruct(sd) => {
                (vec![], Actions::SelfDestruct(SelfdestructWithIndex::new(trace_index, *sd)))
            }
            Action::Reward(_) => (vec![], Actions::Unclassified(trace.clone())),
        };

        if base_action.is_eth_transfer() {
            (pricing, vec![base_action])
        } else {
            let mut res = vec![base_action];
            if let Some(eth) = self.classify_eth_transfer(&trace, trace_index) {
                res.push(eth);
            }

            (pricing, res)
        }
    }

    async fn classify_call(
        &self,
        block: u64,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        if trace.is_static_call() {
            return (vec![], Actions::Unclassified(trace))
        }
        let mut call_info = trace.get_callframe_info();

        // Add logs of delegated calls to the root trace, only if the delegated call is
        // from the same address / in the same call frame.
        if let Action::Call(root_call) = &trace.trace.action {
            let mut delegated_traces = Vec::new();
            collect_delegated_traces(full_trace, &trace.trace.trace_address, &mut delegated_traces);

            for delegated_trace in delegated_traces {
                if let Action::Call(delegated_call) = &delegated_trace.trace.action {
                    if let CallType::DelegateCall = delegated_call.call_type {
                        if delegated_call.from == root_call.to {
                            let logs_internal = delegated_trace.logs.iter().collect::<Vec<&Log>>();
                            call_info.delegate_logs.extend(logs_internal);
                        }
                    }
                }
            }
        }

        if let Some(results) =
            ProtocolClassifications::default().dispatch(call_info, self.libmdbx, block, tx_idx)
        {
            if results.1.is_new_pool() {
                let Actions::NewPool(p) = &results.1 else { unreachable!() };
                self.insert_new_pool(block, p).await;
            } else if results.1.is_pool_config_update() {
                let Actions::PoolConfigUpdate(p) = &results.1 else { unreachable!() };
                if self
                    .libmdbx
                    .insert_pool(block, p.pool_address, p.tokens.as_slice(), None, p.protocol)
                    .await
                    .is_err()
                {
                    error!(pool=?p.pool_address,"failed to update pool config");
                }
            }

            (vec![results.0], results.1)
        } else if let Some(transfer) = self
            .classify_transfer(tx_idx, trace_index, &trace, block)
            .await
        {
            return transfer
        } else {
            return (
                vec![],
                self.classify_eth_transfer(&trace, trace_index)
                    .unwrap_or(Actions::Unclassified(trace)),
            )
        }
    }

    async fn classify_transfer(
        &self,
        tx_idx: u64,
        trace_idx: u64,
        trace: &TransactionTraceWithLogs,
        block: u64,
    ) -> Option<(Vec<DexPriceMsg>, Actions)> {
        if trace.is_delegate_call() {
            return None
        };

        // Attempt to decode the transfer
        match try_decode_transfer(
            trace_idx,
            trace.get_calldata(),
            trace.get_from_addr(),
            trace.get_to_address(),
            self.libmdbx,
            &self.provider,
            block,
            trace.get_msg_value(),
        )
        .await
        {
            Ok(mut transfer) => {
                // go through the log to look for discrepancy of transfer amount
                for log in &trace.logs {
                    if let Some((addr, from, to, amount)) = decode_transfer(log) {
                        if addr != transfer.token.address
                            || transfer.from != from
                            || transfer.to != to
                        {
                            continue
                        }

                        let decimals = transfer.token.decimals;
                        let log_am = amount.to_scaled_rational(decimals);

                        if log_am != transfer.amount {
                            let transferred_amount = min(&log_am, &transfer.amount).clone();
                            let fee = (&log_am - &transfer.amount).abs();
                            transfer.amount = transferred_amount;
                            transfer.fee = fee;
                        }
                        break
                    }
                }

                // Return the adjusted transfer as an action
                Some((
                    vec![DexPriceMsg::Update(brontes_pricing::types::PoolUpdate {
                        block,
                        tx_idx,
                        logs: vec![],
                        action: Actions::Transfer(transfer.clone()),
                    })],
                    Actions::Transfer(transfer),
                ))
            }
            Err(_) => {
                for log in &trace.logs {
                    if let Some((addr, from, to, amount)) = decode_transfer(log) {
                        if self.libmdbx.try_fetch_token_info(addr).is_err() {
                            load_missing_token_info(&self.provider, self.libmdbx, block, addr).await
                        }

                        let token_info = self.libmdbx.try_fetch_token_info(addr).ok()?;
                        let amount = amount.to_scaled_rational(token_info.decimals);
                        let transfer = NormalizedTransfer {
                            amount,
                            token: token_info,
                            to,
                            from,
                            fee: Rational::ZERO,
                            trace_index: trace_idx,
                        };

                        return Some((
                            vec![DexPriceMsg::Update(brontes_pricing::types::PoolUpdate {
                                block,
                                tx_idx,
                                logs: vec![],
                                action: Actions::Transfer(transfer.clone()),
                            })],
                            Actions::Transfer(transfer),
                        ))
                    }
                }
                None
            }
        }
    }

    fn classify_eth_transfer(
        &self,
        trace: &TransactionTraceWithLogs,
        trace_index: u64,
    ) -> Option<Actions> {
        (trace.get_msg_value() > U256::ZERO).then(|| {
            Actions::EthTransfer(NormalizedEthTransfer {
                from: trace.get_from_addr(),
                to: trace.get_to_address(),
                value: trace.get_msg_value(),
                trace_index,
                coinbase_transfer: false,
            })
        })
    }

    async fn classify_create(
        &self,
        block: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Actions>,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        let from_address = trace.get_from_addr();
        let created_addr = trace.get_create_output();

        // get the immediate parent node of this create action so that we can decode the
        // deployment function params
        let node_data = match root_head {
            Some(head) => head.get_immediate_parent_node(trace_index - 1),
            None => return (vec![], Actions::Unclassified(trace)),
        };
        let Some(node_data) = node_data else {
            debug!(block, tx_idx, "failed to find create parent node");
            return (vec![], Actions::Unclassified(trace));
        };

        let Some(calldata) = node_data_store
            .get_ref(node_data.data)
            .and_then(|node| node.first())
            .and_then(|res| res.get_calldata())
        else {
            return (vec![], Actions::Unclassified(trace));
        };

        (
            join_all(
                DiscoveryProtocols::default()
                    .dispatch(
                        self.provider.clone(),
                        from_address,
                        created_addr,
                        trace_index,
                        calldata,
                    )
                    .await
                    .into_iter()
                    // insert the pool returning if it has token values.
                    .filter(|pool| !self.contains_pool(pool.pool_address))
                    .map(|pool| async {
                        self.insert_new_pool(block, &pool).await;
                        pool.try_into().ok()
                    }),
            )
            .await
            .into_iter()
            .flatten()
            .map(DexPriceMsg::DiscoveredPool)
            .collect_vec(),
            Actions::Unclassified(trace),
        )
    }

    async fn insert_new_pool(&self, block: u64, pool: &NormalizedNewPool) {
        if self
            .libmdbx
            .insert_pool(block, pool.pool_address, &pool.tokens, None, pool.protocol)
            .await
            .is_err()
        {
            error!(pool=?pool.pool_address,"failed to insert discovered pool into libmdbx");
        } else {
            info!(
                "Discovered new {} pool:
                            \nAddress:{}
                            ",
                pool.protocol, pool.pool_address
            );
        }
    }

    pub fn close(&self) {
        self.pricing_update_sender
            .send(DexPriceMsg::Closed)
            .unwrap();
    }

    /// This function is used to finalize the classification of complex actions
    /// that contain nested sub-actions that are required to finalize the higher
    /// level classification (e.g: flashloan actions)
    fn finish_classification(
        &self,
        tree: &mut BlockTree<Actions>,
        further_classification_requests: Vec<
            Option<(usize, Vec<MultiCallFrameClassification<Actions>>)>,
        >,
    ) {
        tree.collect_and_classify(&further_classification_requests)
    }
}

fn collect_delegated_traces<'a>(
    traces: &'a [TransactionTraceWithLogs],
    parent_trace_address: &[usize],
    delegated_traces: &mut Vec<&'a TransactionTraceWithLogs>,
) {
    for trace in traces {
        let subtrace_address = &trace.trace.trace_address;
        if subtrace_address.starts_with(parent_trace_address)
            && subtrace_address.len() == parent_trace_address.len() + 1
        {
            delegated_traces.push(trace);
            collect_delegated_traces(traces, subtrace_address, delegated_traces);
        }
    }
}

pub struct TxTreeResult {
    pub pool_updates: Vec<DexPriceMsg>,
    pub further_classification_requests: Option<(usize, Vec<MultiFrameRequest>)>,
    pub root: Root<Actions>,
}
