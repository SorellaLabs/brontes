use std::sync::Arc;

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_pricing::errors::AmmError;
use brontes_types::{
    mev::{Bundle, BundleData, BundleHeader, Liquidation, MevType, TokenProfit, TokenProfits},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use reth_primitives::U256;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};
pub struct BuilderProfitInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> BuilderProfitInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }

    pub async fn calculate_builder_profit<M: TracingProvider>(
        &self,
        builder_address: Address,
        middleware: Arc<M>,
        block_number: Option<u64>,
    ) -> Result<U256, AmmError> {
        let builder_profit;
        let builder_collateral_address = self
            .inner
            .db
            .get_builder_info(builder_address)?
            .unwrap()
            .ultrasound_relay_collateral_address;
        let txn_traces = self.inner.db.load_trace(block_number.unwrap());
        let bid_adjustment = match txn_traces {
            Ok(_) => txn_traces.iter().any(|traces| {
                traces.iter().any(|trace| {
                    trace.trace.iter().any(|trace_with_logs| {
                        trace_with_logs.msg_sender == builder_collateral_address.unwrap()
                    })
                })
            }),
            _ => false,
        };

        let start_builder_balance = middleware
            .get_balance(
                builder_address,
                block_number
                    .map(|num| num.checked_sub(1).unwrap_or(num))
                    .map(Into::into),
            )
            .await?;
        let end_builder_balance = middleware
            .get_balance(builder_address, block_number.map(Into::into))
            .await?;

        if bid_adjustment {
            let start_collateral_balance = middleware
                .get_balance(
                    builder_collateral_address.unwrap(),
                    block_number
                        .map(|num| num.checked_sub(1).unwrap_or(num))
                        .map(Into::into),
                )
                .await?;
            let end_collateral_balance = middleware
                .get_balance(builder_collateral_address.unwrap(), block_number.map(Into::into))
                .await?;
            let bid_adjustment_calcs = start_collateral_balance - end_collateral_balance;
            builder_profit = end_builder_balance - start_builder_balance - bid_adjustment_calcs;
        } else {
            builder_profit = end_builder_balance - start_builder_balance;
        }

        Ok(builder_profit)
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for BuilderProfitInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
    }
}
