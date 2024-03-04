use revm_inspectors::tracing::TracingInspectorConfig;
use reth_rpc_types::TransactionInfo;
use reth_primitives::BlockId;
use reth_provider::StateProviderBox;
use reth_revm::database::StateProviderDatabase;
use inspector::BrontesTracingInspector;
use jsonrpsee::proc_macros::rpc;
use reth_rpc::eth::error::EthResult;
use revm::{
    db::CacheDB,
    primitives::{
        ExecutionResult, State
    }
};
use super::inspector;
/// Custom cli args extension that adds one flag to reth default CLI, this will enable brontes cli extensions and allow installation of brontes inspectors
#[derive(Debug, Clone, Copy, Default, clap::Args)]
struct RethCliBrontesExt {
    /// CLI flag to enable the brontesExt extension namespace
    #[arg(long)]
    pub enable_ext: bool,
}

/// trait interface for a custom rpc namespace: `brontesExt`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[cfg_attr(not(test), rpc(server, namespace = "brontesExt"))]
#[cfg_attr(test, rpc(server, client, namespace = "brontesExt"))]
pub trait BrontesExtApi {
    /// gets all transaction traces
    #[method(name = "getTransactionTraces")]
    async fn get_txn_traces<F, R>(
        &self,
        block_id: BlockId,
        f: F,
    ) -> EthResult<Option<Vec<R>>>
    where
        // This is the callback that's invoked for each transaction with
        F: for<'a> Fn(
                TransactionInfo,
                BrontesTracingInspector,
                ExecutionResult,
                &'a State,
                &'a CacheDB<StateProviderDatabase<StateProviderBox>>,
            ) -> EthResult<R>
            + Send
            + 'static,
        R: Send + 'static;
    
}

impl <F, R>BrontesExtApiServer for BrontesTracingInspector

{
    async fn get_txn_traces(
        &self,
        block_id: BlockId,
        f: F,
    ) -> EthResult<Option<Vec<R>>>
    where
        // This is the callback that's invoked for each transaction with
        F: for<'a> Fn(
                TransactionInfo,
                BrontesTracingInspector,
                ExecutionResult,
                &'a State,
                &'a CacheDB<StateProviderDatabase<StateProviderBox>>,
            ) -> EthResult<R>
            + Send
            + 'static,
        R: Send + 'static, {
            let config = TracingInspectorConfig {
                record_logs:              true,
                record_steps:             false,
                record_state_diff:        false,
                record_stack_snapshots:   reth_revm::tracing::StackSnapshotType::None,
                record_memory_snapshots:  false,
                record_call_return_data:  true,
                exclude_precompile_calls: true,
            };
               unimplemented!()
        }
        
    }