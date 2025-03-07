use std::cmp::min;

use alloy_primitives::{Address, BlockNumber, Bytes, StorageValue, TxHash, B256, U256};
use alloy_rpc_types::{
    state::StateOverride, AnyReceiptEnvelope, BlockId, BlockNumberOrTag, BlockOverrides, Header,
    Log, ReceiptEnvelope, TransactionReceipt, TransactionRequest,
};
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use eyre::eyre;
use reth_primitives::Bytecode;
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_revm::{database::StateProviderDatabase, db::CacheDB};
use reth_rpc_api::EthApiServer;
// use reth_rpc_eth_types::{
//      AnyReceiptEnvelope, BlockId, BlockNumberOrTag, BlockOverrides,
//     EthApiError, EthResult, EthTransactions, Log, RevertError, RpcInvalidTransactionError,
//     TransactionReceipt, TransactionRequest,
// };
use reth_rpc_eth_types::{EthApiError, EthResult, RevertError, RpcInvalidTransactionError};
use revm::Database;
use revm_primitives::ExecutionResult;

use crate::TracingClient;

#[async_trait::async_trait]
impl TracingProvider for TracingClient {
    async fn eth_call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes> {
        // NOTE: these types are equivalent, however we want ot
        EthApiServer::call(&self.api, request, block_number, state_overrides, block_overrides)
            .await
            .map_err(Into::into)
    }

    async fn eth_call_light(
        &self,
        request: TransactionRequest,
        block_number: BlockId,
    ) -> eyre::Result<Bytes> {
        let (cfg, block_env, at) = self.api.evm_env_at(block_number).await?;
        let state = self.api.state_at(at)?;
        let mut db = CacheDB::new(StateProviderDatabase::new(state));
        let env = prepare_call_env(cfg, block_env, request, self.api.call_gas_limit(), &mut db)?;
        let (res, _) = self.api.transact(&mut db, env)?;

        Ok(ensure_success(res.result)?)
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.trace
            .provider()
            .block_hash_for_id(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .map_err(Into::into)
    }

    #[cfg(feature = "local-reth")]
    fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .last_block_number()
            .map_err(Into::into)
    }

    #[cfg(not(feature = "local-reth"))]
    async fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .last_block_number()
            .map_err(Into::into)
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>> {
        self.replay_block_transactions_with_inspector(block_id)
            .await
            .map_err(Into::into)
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<ReceiptEnvelope<Log>>>> {
        Ok(self
            .api
            .block_receipts(BlockId::Number(number))
            .await?
            .map(|t| t.into_iter().map(|t| t.inner).collect::<Vec<_>>()))
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let Some(tx) = EthApiServer::transaction_by_hash(&self.api, hash).await? else {
            return Err(eyre!("no transaction found"));
        };

        Ok((tx.block_number.unwrap(), tx.transaction_index.unwrap() as usize))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        self.trace
            .provider()
            .header_by_number(number)
            .map_err(Into::into)
    }

    // DB Access Methods
    async fn get_storage(
        &self,
        block_number: Option<u64>,
        address: Address,
        storage_key: B256,
    ) -> eyre::Result<Option<StorageValue>> {
        let provider = match block_number {
            Some(block_number) => self.provider_factory.history_by_block_number(block_number),
            None => self.provider_factory.latest(),
        }?;

        let storage_value = provider.storage(address, storage_key)?;

        Ok(storage_value)
    }

    async fn get_bytecode(
        &self,
        block_number: Option<u64>,
        address: Address,
    ) -> eyre::Result<Option<Bytecode>> {
        let provider = match block_number {
            Some(block_number) => self.provider_factory.history_by_block_number(block_number),
            None => self.provider_factory.latest(),
        }?;

        let bytecode = provider.account_code(&address)?;

        Ok(bytecode)
    }
}

pub(crate) fn prepare_call_env<DB>(
    mut cfg: CfgEnvWithHandlerCfg,
    block: BlockEnv,
    request: TransactionRequest,
    gas_limit: u64,
    db: &mut CacheDB<DB>,
) -> EthResult<EnvWithHandlerCfg>
where
    DB: DatabaseRef,
    EthApiError: From<<DB as DatabaseRef>::Error>,
{
    // we want to disable this in eth_call, since this is common practice used by
    // other node impls and providers <https://github.com/foundry-rs/foundry/issues/4388>
    cfg.disable_block_gas_limit = true;

    // Disabled because eth_call is sometimes used with eoa senders
    // See <https://github.com/paradigmxyz/reth/issues/1959>
    cfg.disable_eip3607 = true;

    // The basefee should be ignored for eth_call
    // See:
    // <https://github.com/ethereum/go-ethereum/blob/ee8e83fa5f6cb261dad2ed0a7bbcde4930c41e6c/internal/ethapi/api.go#L985>
    cfg.disable_base_fee = true;

    let request_gas = request.gas;
    let mut env = build_call_evm_env(cfg, block, request)?;
    // set nonce to None so that the next nonce is used when transacting the call
    env.tx.nonce = None;

    if request_gas.is_none() {
        // No gas limit was provided in the request, so we need to cap the transaction
        // gas limit
        if env.tx.gas_price > U256::ZERO {
            // If gas price is specified, cap transaction gas limit with caller allowance
            cap_tx_gas_limit_with_caller_allowance(db, &mut env.tx)?;
        } else {
            // If no gas price is specified, use maximum allowed gas limit. The reason for
            // this is that both Erigon and Geth use pre-configured gas cap even
            // if it's possible to derive the gas limit from the block:
            // <https://github.com/ledgerwatch/erigon/blob/eae2d9a79cb70dbe30b3a6b79c436872e4605458/cmd/rpcdaemon/commands/trace_adhoc.go#L956
            // https://github.com/ledgerwatch/erigon/blob/eae2d9a79cb70dbe30b3a6b79c436872e4605458/eth/ethconfig/config.go#L94>
            env.tx.gas_limit = gas_limit;
        }
    }

    Ok(env)
}

pub(crate) fn build_call_evm_env(
    cfg: CfgEnvWithHandlerCfg,
    block: BlockEnv,
    request: TransactionRequest,
) -> EthResult<EnvWithHandlerCfg> {
    let tx = create_txn_env(&block, request)?;
    Ok(EnvWithHandlerCfg::new_with_cfg_env(cfg, block, tx))
}

pub(crate) fn create_txn_env(
    block_env: &BlockEnv,
    request: TransactionRequest,
) -> EthResult<TxEnv> {
    // Ensure that if versioned hashes are set, they're not empty
    if request
        .blob_versioned_hashes
        .as_ref()
        .map_or(false, |hashes| hashes.is_empty())
    {
        return Err(RpcInvalidTransactionError::BlobTransactionMissingBlobHashes.into())
    }

    let TransactionRequest {
        from,
        to,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        gas,
        value,
        input,
        nonce,
        access_list,
        chain_id,
        blob_versioned_hashes,
        max_fee_per_blob_gas,
        ..
    } = request;

    let CallFees { max_priority_fee_per_gas, gas_price, max_fee_per_blob_gas } =
        CallFees::ensure_fees(
            gas_price.map(U256::from),
            max_fee_per_gas.map(U256::from),
            max_priority_fee_per_gas.map(U256::from),
            block_env.basefee,
            blob_versioned_hashes.as_deref(),
            max_fee_per_blob_gas.map(U256::from),
            block_env.get_blob_gasprice().map(U256::from),
        )?;

    let gas_limit = gas.unwrap_or_else(|| block_env.gas_limit.min(U256::from(u64::MAX)).to());
    let env = TxEnv {
        gas_limit: gas_limit
            .try_into()
            .map_err(|_| RpcInvalidTransactionError::GasUintOverflow)?,
        nonce,
        caller: from.unwrap_or_default(),
        gas_price,
        gas_priority_fee: max_priority_fee_per_gas,
        transact_to: to.map(TransactTo::Call).unwrap_or_else(TransactTo::create),
        value: value.unwrap_or_default(),
        data: input.try_into_unique_input()?.unwrap_or_default(),
        chain_id,
        access_list: access_list
            .map(reth_rpc_types::AccessList::into_flattened)
            .unwrap_or_default(),
        // EIP-4844 fields
        blob_hashes: blob_versioned_hashes.unwrap_or_default(),
        max_fee_per_blob_gas,
    };

    Ok(env)
}

/// Caps the configured [TxEnv] `gas_limit` with the allowance of the caller.
pub(crate) fn cap_tx_gas_limit_with_caller_allowance<DB>(db: DB, env: &mut TxEnv) -> EthResult<()>
where
    DB: Database,
    EthApiError: From<<DB as Database>::Error>,
{
    if let Ok(gas_limit) = caller_gas_allowance(db, env)?.try_into() {
        env.gas_limit = gas_limit;
    }

    Ok(())
}

pub(crate) fn caller_gas_allowance<DB>(mut db: DB, env: &TxEnv) -> EthResult<U256>
where
    DB: Database,
    EthApiError: From<<DB as Database>::Error>,
{
    Ok(db
        // Get the caller account.
        .basic(env.caller)?
        // Get the caller balance.
        .map(|acc| acc.balance)
        .unwrap_or_default()
        // Subtract transferred value from the caller balance.
        .checked_sub(env.value)
        // Return error if the caller has insufficient funds.
        .ok_or_else(|| RpcInvalidTransactionError::InsufficientFunds)?
        // Calculate the amount of gas the caller can afford with the specified gas price.
        .checked_div(env.gas_price)
        // This will be 0 if gas price is 0. It is fine, because we check it before.
        .unwrap_or_default())
}

/// Helper type for representing the fees of a [TransactionRequest]
pub(crate) struct CallFees {
    /// EIP-1559 priority fee
    max_priority_fee_per_gas: Option<U256>,
    /// Unified gas price setting
    ///
    /// Will be the configured `basefee` if unset in the request
    ///
    /// `gasPrice` for legacy,
    /// `maxFeePerGas` for EIP-1559
    gas_price:                U256,
    /// Max Fee per Blob gas for EIP-4844 transactions
    max_fee_per_blob_gas:     Option<U256>,
}

// === impl CallFees ===

impl CallFees {
    /// Ensures the fields of a [TransactionRequest] are not conflicting.
    ///
    /// # EIP-4844 transactions
    ///
    /// Blob transactions have an additional fee parameter `maxFeePerBlobGas`.
    /// If the `maxFeePerBlobGas` or `blobVersionedHashes` are set we treat it
    /// as an EIP-4844 transaction.
    ///
    /// Note: Due to the `Default` impl of [BlockEnv] (Some(0)) this assumes the
    /// `block_blob_fee` is always `Some`
    fn ensure_fees(
        call_gas_price: Option<U256>,
        call_max_fee: Option<U256>,
        call_priority_fee: Option<U256>,
        block_base_fee: U256,
        blob_versioned_hashes: Option<&[B256]>,
        max_fee_per_blob_gas: Option<U256>,
        block_blob_fee: Option<U256>,
    ) -> EthResult<CallFees> {
        /// Get the effective gas price of a transaction as specfified in
        /// EIP-1559 with relevant checks.
        fn get_effective_gas_price(
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            block_base_fee: U256,
        ) -> EthResult<U256> {
            match max_fee_per_gas {
                Some(max_fee) => {
                    if max_fee < block_base_fee {
                        // `base_fee_per_gas` is greater than the `max_fee_per_gas`
                        return Err(RpcInvalidTransactionError::FeeCapTooLow.into())
                    }
                    if max_fee < max_priority_fee_per_gas.unwrap_or(U256::ZERO) {
                        return Err(
                            // `max_priority_fee_per_gas` is greater than the `max_fee_per_gas`
                            RpcInvalidTransactionError::TipAboveFeeCap.into(),
                        )
                    }
                    Ok(min(
                        max_fee,
                        block_base_fee
                            .checked_add(max_priority_fee_per_gas.unwrap_or(U256::ZERO))
                            .ok_or_else(|| {
                                EthApiError::from(RpcInvalidTransactionError::TipVeryHigh)
                            })?,
                    ))
                }
                None => Ok(block_base_fee
                    .checked_add(max_priority_fee_per_gas.unwrap_or(U256::ZERO))
                    .ok_or_else(|| EthApiError::from(RpcInvalidTransactionError::TipVeryHigh))?),
            }
        }

        let has_blob_hashes = blob_versioned_hashes
            .as_ref()
            .map(|blobs| !blobs.is_empty())
            .unwrap_or(false);

        match (call_gas_price, call_max_fee, call_priority_fee, max_fee_per_blob_gas) {
            (gas_price, None, None, None) => {
                // either legacy transaction or no fee fields are specified
                // when no fields are specified, set gas price to zero
                let gas_price = gas_price.unwrap_or(U256::ZERO);
                Ok(CallFees {
                    gas_price,
                    max_priority_fee_per_gas: None,
                    max_fee_per_blob_gas: has_blob_hashes.then_some(block_blob_fee).flatten(),
                })
            }
            (None, max_fee_per_gas, max_priority_fee_per_gas, None) => {
                // request for eip-1559 transaction
                let effective_gas_price = get_effective_gas_price(
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    block_base_fee,
                )?;
                let max_fee_per_blob_gas = has_blob_hashes.then_some(block_blob_fee).flatten();

                Ok(CallFees {
                    gas_price: effective_gas_price,
                    max_priority_fee_per_gas,
                    max_fee_per_blob_gas,
                })
            }
            (None, max_fee_per_gas, max_priority_fee_per_gas, Some(max_fee_per_blob_gas)) => {
                // request for eip-4844 transaction
                let effective_gas_price = get_effective_gas_price(
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    block_base_fee,
                )?;
                // Ensure blob_hashes are present
                if !has_blob_hashes {
                    // Blob transaction but no blob hashes
                    return Err(RpcInvalidTransactionError::BlobTransactionMissingBlobHashes.into())
                }

                Ok(CallFees {
                    gas_price: effective_gas_price,
                    max_priority_fee_per_gas,
                    max_fee_per_blob_gas: Some(max_fee_per_blob_gas),
                })
            }
            _ => {
                // this fallback covers incompatible combinations of fields
                Err(EthApiError::ConflictingFeeFieldsInRequest)
            }
        }
    }
}

pub(crate) fn ensure_success(result: ExecutionResult) -> EthResult<Bytes> {
    match result {
        ExecutionResult::Success { output, .. } => Ok(output.into_data()),
        ExecutionResult::Revert { output, .. } => {
            Err(RpcInvalidTransactionError::Revert(RevertError::new(output)).into())
        }
        ExecutionResult::Halt { .. } => {
            Err(RpcInvalidTransactionError::Revert(RevertError::new(Bytes::new())).into())
        }
    }
}
