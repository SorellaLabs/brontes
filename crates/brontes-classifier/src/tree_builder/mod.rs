use std::{cmp::min, sync::Arc};

use alloy_primitives::U256;
use brontes_types::{
    normalized_actions::{pool::NormalizedNewPool, NormalizedEthTransfer},
    tree::root::NodeData,
    ToScaledRational,
};
mod tree_pruning;
mod utils;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::{Actions, NormalizedAction, SelfdestructWithIndex},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use futures::future::join_all;
use itertools::Itertools;
use malachite::num::arithmetic::traits::Abs;
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::Action;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};
use tree_pruning::account_for_tax_tokens;
use utils::{decode_transfer, get_coinbase_transfer};

use self::transfer::try_decode_transfer;
use crate::{
    classifiers::{DiscoveryProtocols, *},
    ActionCollection, FactoryDiscoveryDispatch,
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
    ) -> BlockTree<Actions> {
        let tx_roots = self.build_all_tx_trees(traces, &header).await;
        let mut tree = BlockTree::new(header, tx_roots.len());

        // send out all updates
        let further_classification_requests = self.process_tx_roots(tx_roots, &mut tree);

        Self::prune_tree(&mut tree);
        self.finish_classification(&mut tree, further_classification_requests);

        tree.finalize_tree();

        tree
    }

    fn process_tx_roots(
        &self,
        tx_roots: Vec<TxTreeResult>,
        tree: &mut BlockTree<Actions>,
    ) -> Vec<Option<(usize, Vec<u64>)>> {
        tx_roots
            .into_iter()
            .map(|root_data| {
                tree.insert_root(root_data.root);
                root_data.pool_updates.into_iter().for_each(|update| {
                    tracing::debug!("sending update");
                    self.pricing_update_sender.send(update).unwrap();
                });
                root_data.further_classification_requests
            })
            .collect_vec()
    }

    pub(crate) fn prune_tree(tree: &mut BlockTree<Actions>) {
        // tax token accounting should always be first.
        account_for_tax_tokens(tree);
        // remove_swap_transfers(tree);
        // remove_mint_transfers(tree);
        // remove_collect_transfers(tree);
    }

    pub(crate) async fn build_all_tx_trees(
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
                        return None;
                    }
                    // post classification processing collectors
                    let mut further_classification_requests = Vec::new();
                    let mut pool_updates: Vec<DexPriceMsg> = Vec::new();

                    let root_trace = trace.trace.remove(0);
                    let address = root_trace.get_from_addr();
                    let classification = self
                        .process_classification(
                            header.number,
                            None,
                            &NodeData(vec![]),
                            tx_idx as u64,
                            0,
                            root_trace,
                            &mut further_classification_requests,
                            &mut pool_updates,
                        )
                        .await;

                    let node = Node::new(0, address, vec![]);

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
                        data_store:  NodeData(vec![Some(classification)]),
                    };

                    for (index, trace) in trace.trace.into_iter().enumerate() {
                        if let Some(coinbase) = &mut tx_root.gas_details.coinbase_transfer {
                            *coinbase +=
                                get_coinbase_transfer(header.beneficiary, &trace.trace.action)
                                    .unwrap_or_default()
                        } else {
                            tx_root.gas_details.coinbase_transfer =
                                get_coinbase_transfer(header.beneficiary, &trace.trace.action);
                        }

                        let classification = self
                            .process_classification(
                                header.number,
                                Some(&tx_root.head),
                                &tx_root.data_store,
                                tx_idx as u64,
                                (index + 1) as u64,
                                trace.clone(),
                                &mut further_classification_requests,
                                &mut pool_updates,
                            )
                            .await;

                        let from_addr = trace.get_from_addr();

                        let node =
                            Node::new((index + 1) as u64, from_addr, trace.trace.trace_address);

                        tx_root.insert(node, classification);
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
        further_classification_requests: &mut Vec<u64>,
        pool_updates: &mut Vec<DexPriceMsg>,
    ) -> Actions {
        let (update, classification) = self
            .classify_node(block_number, root_head, node_data_store, tx_index, trace, trace_index)
            .await;

        // Here we are marking more complex actions that require data
        // that can only be retrieved by classifying it's action and
        // all subsequent child actions.
        if classification.continue_classification() {
            further_classification_requests.push(classification.get_trace_index());
        }

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
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        if trace.trace.error.is_some() {
            return (vec![], Actions::Revert);
        }
        match trace.action_type() {
            Action::Call(_) => self.classify_call(block, tx_idx, trace, trace_index).await,
            Action::Create(_) => {
                self.classify_create(block, root_head, node_data_store, tx_idx, trace, trace_index)
                    .await
            }
            Action::Selfdestruct(sd) => {
                (vec![], Actions::SelfDestruct(SelfdestructWithIndex::new(trace_index, *sd)))
            }
            Action::Reward(_) => (vec![], Actions::Unclassified(trace)),
        }
    }

    async fn classify_call(
        &self,
        block: u64,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        if trace.is_static_call() {
            return (vec![], Actions::Unclassified(trace));
        }
        let call_info = trace.get_callframe_info();

        if let Some(results) =
            ProtocolClassifications::default().dispatch(call_info, self.libmdbx, block, tx_idx)
        {
            (vec![results.0], results.1)
        } else if let Some(transfer) = self.classify_transfer(trace_index, &trace, block).await {
            return transfer;
        } else {
            return (vec![], self.classify_eth_transfer(trace, trace_index));
        }
    }

    async fn classify_transfer(
        &self,
        trace_idx: u64,
        trace: &TransactionTraceWithLogs,
        block: u64,
    ) -> Option<(Vec<DexPriceMsg>, Actions)> {
        // Determine the appropriate address based on whether it's a delegate call
        let token_address =
            if trace.is_delegate_call() { trace.get_from_addr() } else { trace.get_to_address() };

        // Attempt to decode the transfer
        match try_decode_transfer(
            trace_idx,
            trace.get_calldata(),
            trace.get_from_addr(),
            token_address,
            self.libmdbx,
            &self.provider,
            block,
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
                            continue;
                        }

                        let decimals = transfer.token.decimals;
                        let log_am = amount.to_scaled_rational(decimals);

                        if log_am != transfer.amount {
                            let transferred_amount = min(&log_am, &transfer.amount).clone();
                            let fee = (&log_am - &transfer.amount).abs();
                            transfer.amount = transferred_amount;
                            transfer.fee = fee;
                        }
                        break;
                    }
                }

                // Return the adjusted transfer as an action
                Some((vec![], Actions::Transfer(transfer)))
            }
            Err(_) => None,
        }
    }

    fn classify_eth_transfer(&self, trace: TransactionTraceWithLogs, trace_index: u64) -> Actions {
        if trace.get_calldata().is_empty() && trace.get_msg_value() > U256::ZERO {
            Actions::EthTransfer(NormalizedEthTransfer {
                from: trace.get_from_addr(),
                to: trace.get_to_address(),
                value: trace.get_msg_value(),
                trace_index,
            })
        } else {
            Actions::Unclassified(trace)
        }
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
            .insert_pool(block, pool.pool_address, [pool.tokens[0], pool.tokens[1]], pool.protocol)
            .await
            .is_err()
        {
            error!(pool=?pool.pool_address,"failed to insert discovered pool into libmdbx");
        } else {
            info!(
                "Discovered new {} pool: 
                            \nAddress:{} 
                            \nToken 0: {}
                            \nToken 1: {}",
                pool.protocol, pool.pool_address, pool.tokens[0], pool.tokens[1]
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
        further_classification_requests: Vec<Option<(usize, Vec<u64>)>>,
    ) {
        tree.collect_and_classify(&further_classification_requests)
    }
}

pub struct TxTreeResult {
    pub pool_updates: Vec<DexPriceMsg>,
    pub further_classification_requests: Option<(usize, Vec<u64>)>,
    pub root: Root<Actions>,
}
