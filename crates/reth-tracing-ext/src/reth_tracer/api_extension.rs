


/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
struct RethCliTxpoolExt {
    /// CLI flag to enable the txpool extension namespace
    #[arg(long)]
    pub enable_ext: bool,
}

/// trait interface for a custom rpc namespace: `txpool`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[cfg_attr(not(test), rpc(server, namespace = "txpoolExt"))]
#[cfg_attr(test, rpc(server, client, namespace = "txpoolExt"))]
pub trait TxpoolExtApi {
    /// Returns the number of transactions in the pool.
    #[method(name = "transactionCount")]
    async fn get_txn_trace() -> RpcResult<usize>;
}

/// The type that implements the `txpool` rpc namespace trait
pub struct TxpoolExt<Pool> {
    pool: Pool,
}

impl<Pool> TxpoolExtApiServer for TxpoolExt<Pool>
where
    Pool: TransactionPool + Clone + 'static,
{
    fn transaction_count(&self) -> RpcResult<usize> {
        Ok(self.pool.pool_size().total)
    }
}

async fn trace_block_with<F, R>(
    &self,
    block_id: BlockId,
    config: TracingInspectorConfig,
    f: F,
) -> EthResult<Option<Vec<R>>>
where
    // This is the callback that's invoked for each transaction with
    F: for<'a> Fn(
            TransactionInfo,
            TracingInspector,
            ExecutionResult,
            &'a State,
            &'a CacheDB<StateProviderDatabase<StateProviderBox>>,
        ) -> EthResult<R>
        + Send
        + 'static,
    R: Send + 'static,
{
    self.trace_block_until(block_id, None, config, f).await
}

async fn trace_block_until<F, R>(
    &self,
    block_id: BlockId,
    highest_index: Option<u64>,
    config: TracingInspectorConfig,
    f: F,
) -> EthResult<Option<Vec<R>>>
where
    F: for<'a> Fn(
            TransactionInfo,
            TracingInspector,
            ExecutionResult,
            &'a State,
            &'a CacheDB<StateProviderDatabase<StateProviderBox>>,
        ) -> EthResult<R>
        + Send
        + 'static,
    R: Send + 'static,
{
    let ((cfg, block_env, _), block) =
        futures::try_join!(self.evm_env_at(block_id), self.block_with_senders(block_id))?;

    let Some(block) = block else { return Ok(None) };

    // replay all transactions of the block
    self.spawn_tracing_task_with(move |this| {
        // we need to get the state of the parent block because we're replaying this block on
        // top of its parent block's state
        let state_at = block.parent_hash;
        let block_hash = block.hash();

        let block_number = block_env.number.saturating_to::<u64>();
        let base_fee = block_env.basefee.saturating_to::<u64>();

        // prepare transactions, we do everything upfront to reduce time spent with open state
        let max_transactions =
            highest_index.map_or(block.body.len(), |highest| highest as usize);
        let mut results = Vec::with_capacity(max_transactions);

        let mut transactions = block
            .into_transactions_ecrecovered()
            .take(max_transactions)
            .enumerate()
            .map(|(idx, tx)| {
                let tx_info = TransactionInfo {
                    hash: Some(tx.hash()),
                    index: Some(idx as u64),
                    block_hash: Some(block_hash),
                    block_number: Some(block_number),
                    base_fee: Some(base_fee),
                };
                let tx_env = tx_env_with_recovered(&tx);
                (tx_info, tx_env)
            })
            .peekable();

        // now get the state
        let state = this.state_at(state_at.into())?;
        let mut db = CacheDB::new(StateProviderDatabase::new(state));

        while let Some((tx_info, tx)) = transactions.next() {
            let env = EnvWithHandlerCfg::new_with_cfg_env(cfg.clone(), block_env.clone(), tx);

            let mut inspector = TracingInspector::new(config);
            let (res, _) = inspect(&mut db, env, &mut inspector)?;
            let ResultAndState { result, state } = res;
            results.push(f(tx_info, inspector, result, &state, &db)?);

            // need to apply the state changes of this transaction before executing the
            // next transaction
            if transactions.peek().is_some() {
                // need to apply the state changes of this transaction before executing
                // the next transaction
                db.commit(state)
            }
        }

        Ok(results)
    })
    .await
    .map(Some)
}