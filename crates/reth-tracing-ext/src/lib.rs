use std::{fmt::Debug, path::Path, sync::Arc};

use alloy_primitives::Log;
use brontes_types::structured_trace::{TransactionTraceWithLogs, TxTrace};
use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    externals::TreeExternals, BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree,
};
use reth_db::DatabaseEnv;
use reth_network_api::noop::NoopNetwork;
use reth_primitives::{BlockId, Bytes, PruneModes, MAINNET, U64};
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::{
    inspectors::GasInspector,
    tracing::{
        types::{CallKind, CallTraceNode},
        TracingInspectorConfig, *,
    },
    DatabaseRef, EvmProcessorFactory,
};
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        error::EthResult,
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
        EthFilterConfig, EthTransactions, FeeHistoryCache, FeeHistoryCacheConfig,
        RPC_DEFAULT_GAS_CAP,
    },
    BlockingTaskGuard, BlockingTaskPool, EthApi, EthFilter, TraceApi,
};
use reth_rpc_types::{
    trace::parity::{TransactionTrace, *},
    TransactionInfo,
};
use reth_tasks::TaskManager;
use reth_transaction_pool::{
    blobstore::NoopBlobStore, validate::EthTransactionValidatorBuilder, CoinbaseTipOrdering,
    EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor,
};
use revm::interpreter::InstructionResult;
use revm_primitives::{ExecutionResult, SpecId, KECCAK_EMPTY};
use tokio::runtime::Handle;

mod provider;

pub type Provider = BlockchainProvider<
    Arc<DatabaseEnv>,
    ShareableBlockchainTree<Arc<DatabaseEnv>, EvmProcessorFactory>,
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork>;

pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Provider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore,
>;

#[derive(Debug, Clone)]
pub struct TracingClient {
    pub api:    EthApi<Provider, RethTxPool, NoopNetwork>,
    pub filter: EthFilter<Provider, RethTxPool>,
    pub trace:  TraceApi<Provider, RethApi>,
}

impl TracingClient {
    pub fn new(db_path: &Path, handle: Handle, max_tasks: u64) -> (TaskManager, Self) {
        let task_manager = TaskManager::new(handle);
        let task_executor: reth_tasks::TaskExecutor = task_manager.executor();

        let chain = MAINNET.clone();
        let db = Arc::new(init_db(db_path).unwrap());
        let provider_factory = ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain));

        let tree_externals = TreeExternals::new(
            provider_factory,
            Arc::new(BeaconConsensus::new(Arc::clone(&chain))),
            EvmProcessorFactory::new(chain.clone()),
        );

        let tree_config = BlockchainTreeConfig::default();

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(tree_externals, tree_config, Some(PruneModes::none())).unwrap(),
        );

        let provider = BlockchainProvider::new(
            ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain)),
            blockchain_tree,
        )
        .unwrap();

        let state_cache = EthStateCache::spawn(provider.clone(), EthStateCacheConfig::default());

        let transaction_validator = EthTransactionValidatorBuilder::new(chain.clone())
            .build_with_tasks(provider.clone(), task_executor.clone(), NoopBlobStore::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            transaction_validator,
            NoopBlobStore::default(),
            Default::default(),
        );

        let blocking = BlockingTaskPool::build().unwrap();
        let eth_state_config = EthStateCacheConfig::default();
        let fee_history = FeeHistoryCache::new(
            EthStateCache::spawn(provider.clone(), eth_state_config),
            FeeHistoryCacheConfig::default(),
        );
        // blocking task pool
        // fee history cache
        let api = EthApi::new(
            provider.clone(),
            tx_pool.clone(),
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone(),
            ),
            RPC_DEFAULT_GAS_CAP,
            blocking,
            fee_history,
        );
        let filter_config = EthFilterConfig::default();
        let filter = EthFilter::new(
            provider.clone(),
            tx_pool,
            state_cache.clone(),
            filter_config,
            Box::new(task_executor),
        );

        let tracing_call_guard = BlockingTaskGuard::new(max_tasks as u32);

        let trace = TraceApi::new(provider, api.clone(), tracing_call_guard);

        (task_manager, Self { api, trace, filter })
    }

    /// Replays all transactions in a block
    pub async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> EthResult<Option<Vec<TxTrace>>> {
        let config = TracingInspectorConfig {
            record_logs:              true,
            record_steps:             false,
            record_state_diff:        false,
            record_stack_snapshots:   reth_revm::tracing::StackSnapshotType::None,
            record_memory_snapshots:  false,
            record_call_return_data:  true,
            exclude_precompile_calls: true,
        };

        self.api
            .trace_block_with(block_id, config, move |tx_info, inspector, res, _, _| {
                // this is safe as there the exact same memory layout. This is needed as we need
                // access to the internal fields of the struct that arent public
                let localized: TracingInspectorLocal = unsafe { std::mem::transmute(inspector) };

                Ok(localized.into_trace_results(tx_info, &res))
            })
            .await
    }
}

#[derive(Debug, Clone)]
pub struct TracingInspectorLocal {
    /// Configures what and how the inspector records traces.
    pub _config:                TracingInspectorConfig,
    /// Records all call traces
    pub traces:                 CallTraceArena,
    /// Tracks active calls
    pub _trace_stack:           Vec<usize>,
    /// Tracks active steps
    pub _step_stack:            Vec<StackStep>,
    /// Tracks the return value of the last call
    pub _last_call_return_data: Option<Bytes>,
    /// The gas inspector used to track remaining gas.
    pub _gas_inspector:         GasInspector,
    /// The spec id of the EVM.
    ///
    /// This is filled during execution.
    pub _spec_id:               Option<SpecId>,
}

impl TracingInspectorLocal {
    pub fn into_trace_results(self, info: TransactionInfo, res: &ExecutionResult) -> TxTrace {
        let gas_used = res.gas_used().into();

        let trace = self.build_trace();

        TxTrace {
            trace: trace.unwrap_or_default(),
            tx_hash: info.hash.unwrap(),
            gas_used,
            effective_price: 0,
            tx_index: info.index.unwrap(),
            is_success: res.is_success(),
        }
    }

    fn iter_traceable_nodes(&self) -> impl Iterator<Item = &CallTraceNode> {
        self.traces
            .nodes()
            .iter()
            .filter(|node| !node.trace.maybe_precompile.unwrap_or(false))
    }

    /// Returns the tracing types that are configured in the set.
    ///
    /// Warning: if [TraceType::StateDiff] is provided this does __not__ fill
    /// the state diff, since this requires access to the account diffs.
    ///
    /// See [Self::into_trace_results_with_state] and [populate_state_diff].
    pub fn build_trace(&self) -> Option<Vec<TransactionTraceWithLogs>> {
        if self.traces.nodes().is_empty() {
            return None
        }

        let mut traces = Vec::with_capacity(self.traces.nodes().len());

        for node in self.iter_traceable_nodes() {
            let trace_address = self.trace_address(self.traces.nodes(), node.idx);

            let trace = self.build_tx_trace(node, trace_address);
            let logs = node
                .logs
                .iter()
                .map(|log| Log { address: node.trace.address, data: log.clone() })
                .collect::<Vec<_>>();

            traces.push(TransactionTraceWithLogs {
                trace,
                logs,
                decoded_data: None,
                trace_idx: node.idx as u64,
            });

            // check if the trace node is a selfdestruct
            if node.trace.status == InstructionResult::SelfDestruct {
                // selfdestructs are not recorded as individual call traces but are derived from
                // the call trace and are added as additional `TransactionTrace` objects in the
                // trace array
                let addr = {
                    let last = traces.last_mut().expect("exists");
                    let mut addr = last.trace.trace_address.clone();
                    addr.push(last.trace.subtraces);
                    // need to account for the additional selfdestruct trace
                    last.trace.subtraces += 1;
                    addr
                };

                if let Some(trace) = self.parity_selfdestruct_trace(node, addr) {
                    traces.push(TransactionTraceWithLogs {
                        trace,
                        logs: vec![],
                        decoded_data: None,
                        trace_idx: node.idx as u64,
                    });
                }
            }
        }

        Some(traces)
    }

    fn trace_address(&self, nodes: &[CallTraceNode], idx: usize) -> Vec<usize> {
        if idx == 0 {
            // root call has empty traceAddress
            return vec![]
        }
        let mut graph = vec![];
        let mut node = &nodes[idx];
        if node.trace.maybe_precompile.unwrap_or(false) {
            return graph
        }
        while let Some(parent) = node.parent {
            // the index of the child call in the arena
            let child_idx = node.idx;
            node = &nodes[parent];
            // find the index of the child call in the parent node
            let call_idx = node
                .children
                .iter()
                .position(|child| *child == child_idx)
                .expect("non precompile child call exists in parent");
            graph.push(call_idx);
        }
        graph.reverse();
        graph
    }

    pub(crate) fn parity_selfdestruct_trace(
        &self,
        node: &CallTraceNode,
        trace_address: Vec<usize>,
    ) -> Option<TransactionTrace> {
        let trace = self.parity_selfdestruct_action(node)?;
        Some(TransactionTrace {
            action: trace,
            error: None,
            result: None,
            trace_address,
            subtraces: 0,
        })
    }

    pub(crate) fn parity_selfdestruct_action(&self, node: &CallTraceNode) -> Option<Action> {
        if node.trace.selfdestruct_refund_target.is_some() {
            Some(Action::Selfdestruct(SelfdestructAction {
                address:        node.trace.address,
                refund_address: node.trace.selfdestruct_refund_target.unwrap_or_default(),
                balance:        node.trace.value,
            }))
        } else {
            None
        }
    }

    pub(crate) fn parity_action(&self, node: &CallTraceNode) -> Action {
        match node.trace.kind {
            CallKind::Call | CallKind::StaticCall | CallKind::CallCode | CallKind::DelegateCall => {
                Action::Call(CallAction {
                    from:      node.trace.caller,
                    to:        node.trace.address,
                    value:     node.trace.value,
                    gas:       U64::from(node.trace.gas_limit),
                    input:     node.trace.data.clone(),
                    call_type: node.trace.kind.into(),
                })
            }
            CallKind::Create | CallKind::Create2 => Action::Create(CreateAction {
                from:  node.trace.caller,
                value: node.trace.value,
                gas:   U64::from(node.trace.gas_limit),
                init:  node.trace.data.clone(),
            }),
        }
    }

    pub(crate) fn parity_trace_output(&self, node: &CallTraceNode) -> TraceOutput {
        match node.trace.kind {
            CallKind::Call | CallKind::StaticCall | CallKind::CallCode | CallKind::DelegateCall => {
                TraceOutput::Call(CallOutput {
                    gas_used: U64::from(node.trace.gas_used),
                    output:   node.trace.output.clone(),
                })
            }
            CallKind::Create | CallKind::Create2 => TraceOutput::Create(CreateOutput {
                gas_used: U64::from(node.trace.gas_used),
                code:     node.trace.output.clone(),
                address:  node.trace.address,
            }),
        }
    }

    /// Returns the error message if it is an erroneous result.
    pub(crate) fn as_error_msg(&self, node: &CallTraceNode) -> Option<String> {
        // See also <https://github.com/ethereum/go-ethereum/blob/34d507215951fb3f4a5983b65e127577989a6db8/eth/tracers/native/call_flat.go#L39-L55>
        node.trace.is_error().then(|| match node.trace.status {
            InstructionResult::Revert => "execution reverted".to_string(),
            InstructionResult::OutOfGas | InstructionResult::MemoryOOG => "out of gas".to_string(),
            InstructionResult::OpcodeNotFound => "invalid opcode".to_string(),
            InstructionResult::StackOverflow => "Out of stack".to_string(),
            InstructionResult::InvalidJump => "invalid jump destination".to_string(),
            InstructionResult::PrecompileError => "precompiled failed".to_string(),
            status => format!("{:?}", status),
        })
    }

    pub fn build_tx_trace(
        &self,
        node: &CallTraceNode,
        trace_address: Vec<usize>,
    ) -> TransactionTrace {
        let action = self.parity_action(node);
        let result = if node.trace.is_error() && !node.trace.is_revert() {
            // if the trace is a selfdestruct or an error that is not a revert, the result
            // is None
            None
        } else {
            Some(self.parity_trace_output(node))
        };
        let error = self.as_error_msg(node);
        TransactionTrace { action, error, result, trace_address, subtraces: node.children.len() }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StackStep {
    _trace_idx: usize,
    _step_idx:  usize,
}

/// Opens up an existing database at the specified path.
pub fn init_db<P: AsRef<Path> + Debug>(path: P) -> eyre::Result<DatabaseEnv> {
    reth_db::open_db(path.as_ref(), None)
}

#[inline]
pub(crate) fn load_account_code<DB: DatabaseRef>(
    db: DB,
    db_acc: &revm::primitives::AccountInfo,
) -> Option<Bytes> {
    db_acc
        .code
        .as_ref()
        .map(|code| code.original_bytes())
        .or_else(|| {
            if db_acc.code_hash == KECCAK_EMPTY {
                None
            } else {
                db.code_by_hash_ref(db_acc.code_hash)
                    .ok()
                    .map(|code| code.original_bytes())
            }
        })
        .map(Into::into)
}
