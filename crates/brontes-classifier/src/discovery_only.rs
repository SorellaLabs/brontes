use std::sync::Arc;

use alloy_primitives::Log;
use brontes_core::missing_token_info::load_missing_token_info;
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::{pool::NormalizedNewPool, Action, MultiFrameRequest},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{root::NodeData, GasDetails, Node, Root},
};
use futures::future::join_all;
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::{Action as TraceAction, CallType};
use tracing::{error, trace};

use self::erc20::try_decode_transfer;
use crate::{
    classifiers::*, tree_builder::utils::decode_transfer, ActionCollection,
    FactoryDiscoveryDispatch,
};

#[derive(Debug)]
pub struct DiscoveryOnlyClassifier<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> {
    libmdbx:  &'db DB,
    provider: Arc<T>,
}

impl<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> Clone
    for DiscoveryOnlyClassifier<'db, T, DB>
{
    fn clone(&self) -> Self {
        Self { libmdbx: self.libmdbx, provider: self.provider.clone() }
    }
}

impl<'db, T: TracingProvider, DB: LibmdbxReader + DBWriter> DiscoveryOnlyClassifier<'db, T, DB> {
    pub fn new(libmdbx: &'db DB, provider: Arc<T>) -> Self {
        Self { libmdbx, provider }
    }

    pub async fn run_discovery(&self, traces: Vec<TxTrace>, header: Header) {
        self.process_txs(traces, &header).await;
    }

    pub(crate) async fn process_txs(&self, traces: Vec<TxTrace>, header: &Header) {
        join_all(
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_idx, mut trace)| async move {
                    // here only traces where the root tx failed are filtered out
                    if trace.trace.is_empty() || !trace.is_success {
                        tracing::trace!(
                            empty = trace.trace.is_empty(),
                            is_success = trace.is_success
                        );
                        return
                    }

                    let root_trace = trace.trace.remove(0);
                    let address = root_trace.get_from_addr();
                    let trace_idx = root_trace.trace_idx;

                    self.process_classification(
                        header.number,
                        None,
                        &NodeData(vec![]),
                        tx_idx as u64,
                        trace_idx,
                        root_trace.clone(),
                        &trace.trace,
                    )
                    .await;

                    let node = Node::new(trace_idx, address, vec![]);
                    let action = vec![Action::Unclassified(root_trace)];

                    let mut tx_root = Root {
                        position: tx_idx,
                        head: node,
                        tx_hash: trace.tx_hash,
                        private: false,
                        total_msg_value_transfers: vec![],
                        gas_details: GasDetails {
                            coinbase_transfer:   None,
                            gas_used:            trace.gas_used,
                            effective_gas_price: trace.effective_price,
                            priority_fee:        trace.effective_price
                                - (header.base_fee_per_gas.unwrap_or_default() as u128),
                        },
                        data_store: NodeData(vec![Some(action)]),
                    };

                    let tx_trace = &trace.trace;
                    for trace in &trace.trace {
                        let from_addr = trace.get_from_addr();

                        let node = Node::new(
                            trace.trace_idx,
                            from_addr,
                            trace.trace.trace_address.clone(),
                        );

                        self.process_classification(
                            header.number,
                            Some(&tx_root.head),
                            &tx_root.data_store,
                            tx_idx as u64,
                            trace.trace_idx,
                            trace.clone(),
                            tx_trace,
                        )
                        .await;

                        let action = Action::Unclassified(trace.clone());
                        tx_root.insert(node, vec![action]);
                    }
                }),
        )
        .await;
    }

    async fn process_classification(
        &self,
        block_number: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Action>,
        tx_index: u64,
        trace_index: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
    ) {
        self.classify_node(
            block_number,
            root_head,
            node_data_store,
            tx_index,
            trace,
            full_trace,
            trace_index,
        )
        .await;
    }

    fn contains_pool(&self, address: Address) -> bool {
        self.libmdbx.get_protocol(address).is_ok()
    }

    async fn classify_node(
        &self,
        block: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Action>,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
        trace_index: u64,
    ) {
        if trace.trace.error.is_some() {
            return
        }
        match trace.action_type() {
            TraceAction::Call(_) => {
                self.classify_call(block, tx_idx, trace.clone(), full_trace, trace_index)
                    .await
            }
            TraceAction::Create(_) => {
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
            _ => {}
        };
    }

    async fn classify_call(
        &self,
        block: u64,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        full_trace: &[TransactionTraceWithLogs],
        trace_index: u64,
    ) {
        if trace.is_static_call() {
            return
        }

        let mut call_info = trace.get_callframe_info();
        // Add logs of delegated calls to the root trace, only if the delegated call is
        // from the same address / in the same call frame.
        if let TraceAction::Call(root_call) = &trace.trace.action {
            let mut delegated_traces = Vec::new();
            collect_delegated_traces(full_trace, &trace.trace.trace_address, &mut delegated_traces);

            for delegated_trace in delegated_traces {
                if let TraceAction::Call(delegated_call) = &delegated_trace.trace.action {
                    if let CallType::DelegateCall = delegated_call.call_type {
                        if delegated_call.from == root_call.to {
                            let logs_internal = delegated_trace.logs.iter().collect::<Vec<&Log>>();
                            call_info.delegate_logs.extend(logs_internal);
                        }
                    }
                }
            }
        }

        if let Some(results) = ProtocolClassifier::default().dispatch(
            call_info,
            self.libmdbx,
            block,
            tx_idx,
            self.provider.clone(),
        ) {
            if results.1.is_new_pool() {
                let Action::NewPool(p) = &results.1 else { unreachable!() };
                self.insert_new_pool(block, p.clone()).await;
            } else if results.1.is_pool_config_update() {
                let Action::PoolConfigUpdate(p) = &results.1 else { unreachable!() };
                if self
                    .libmdbx
                    .insert_pool(block, p.pool_address, p.tokens.as_slice(), None, p.protocol)
                    .await
                    .is_err()
                {
                    error!(pool=?p.pool_address,"failed to update pool config");
                }
            }
        } else {
            self.classify_transfer(trace_index, &trace, block).await
        }
    }

    async fn classify_transfer(
        &self,
        trace_idx: u64,
        trace: &TransactionTraceWithLogs,
        block: u64,
    ) {
        if trace.is_delegate_call() {
            return
        };

        // Attempt to decode the transfer
        if try_decode_transfer(
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
        .is_err()
        {
            for log in &trace.logs {
                if let Some((addr, ..)) = decode_transfer(log) {
                    if self.libmdbx.try_fetch_token_info(addr).is_err() {
                        load_missing_token_info(&self.provider, self.libmdbx, block, addr).await
                    }
                }
            }
        }
    }

    async fn classify_create(
        &self,
        block: u64,
        root_head: Option<&Node>,
        node_data_store: &NodeData<Action>,
        _tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) {
        let created_addr = trace.get_create_output();

        // get the immediate parent node of this create action so that we can decode the
        // deployment function params
        let mut all_nodes = Vec::new();

        match root_head {
            Some(head) => {
                let mut start_index = 0u64;
                head.get_last_create_call(&mut start_index, node_data_store);
                head.get_all_parent_nodes_for_discovery(&mut all_nodes, start_index, trace_index)
            }
            None => return,
        };

        let search_data = all_nodes
            .iter()
            .filter_map(|node| {
                node_data_store
                    .get_ref(node.data)
                    .and_then(|node| node.first())
            })
            .filter_map(|node_data| Some((node_data.get_from_address(), node_data.get_calldata()?)))
            .collect::<Vec<_>>();

        if search_data.is_empty() {
            return
        }

        join_all(
            DiscoveryClassifier::default()
                .dispatch(self.provider.clone(), search_data, created_addr, trace_index)
                .await
                .into_iter()
                // insert the pool returning if it has token values.
                .filter(|pool| !self.contains_pool(pool.pool_address))
                .map(|pool| async move { self.insert_new_pool(block, pool).await }),
        )
        .await;
    }

    async fn insert_new_pool(&self, block: u64, pool: NormalizedNewPool) {
        if self
            .libmdbx
            .insert_pool(block, pool.pool_address, &pool.tokens, None, pool.protocol)
            .await
            .is_err()
        {
            error!(pool=?pool.pool_address,"failed to insert discovered pool into libmdbx");
        } else {
            trace!(
                "Discovered new {} pool:
                            \nAddress:{}
                            ",
                pool.protocol,
                pool.pool_address
            );
        }
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
    pub root: Root<Action>,
}
