use std::sync::Arc;

use brontes_database::{Metadata, Pair};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{ClassifiedMev, Liquidation, MevType, SpecificMev},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    tree::{BlockTree, GasDetails, Node, Root},
    ToFloatNearest,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector};

pub struct LiquidationInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> LiquidationInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for LiquidationInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let liq_txs = tree.collect_all(|node| {
            (
                node.data.is_liquidation(),
                node.subactions.iter().any(|action| action.is_liquidation()),
            )
        });

        liq_txs
            .into_par_iter()
            .filter_map(|(tx_hash, liq)| {
                let root = tree.get_root(tx_hash)?;
                let eoa = root.head.address;
                let mev_contract = root.head.data.get_to_address();
                let idx = root.get_block_position();
                let gas_details = tree.get_gas_details(tx_hash)?;

                self.calculate_liquidation(
                    tx_hash,
                    idx,
                    mev_contract,
                    eoa,
                    metadata.clone(),
                    liq,
                    gas_details,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl LiquidationInspector<'_> {
    fn calculate_liquidation(
        &self,
        tx_hash: B256,
        idx: usize,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<Metadata>,
        actions: Vec<Actions>,
        gas_details: &GasDetails,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let swaps = actions
            .iter()
            .filter_map(|action| if let Actions::Swap(swap) = action { Some(swap) } else { None })
            .collect::<Vec<_>>();

        let liqs = actions
            .iter()
            .filter_map(
                |action| {
                    if let Actions::Liquidation(liq) = action {
                        Some(liq)
                    } else {
                        None
                    }
                },
            )
            .collect::<Vec<_>>();

        let gas_finalized = metadata.get_gas_price_usd(gas_details.gas_paid());

        let mev = ClassifiedMev {
            block_number: metadata.block_num,
            eoa,
            tx_hash,
            mev_contract,
            mev_profit_collector: todo!(),
            finalized_profit_usd: todo!(),
            finalized_bribe_usd: gas_finalized.to_float(),
            mev_type: MevType::Liquidation,
        };

        // TODO: filter swaps not related to liqs?
        let new_liquidation = Liquidation {
            liquidation_tx_hash: tx_hash,
            trigger: todo!(),
            liquidation_swaps_index: swaps.iter().map(|s| s.trace_index).collect::<Vec<_>>(),
            liquidation_swaps_from: swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            liquidation_swaps_pool: swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            liquidation_swaps_token_in: swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            liquidation_swaps_token_out: swaps.iter().map(|s| s.token_out).collect::<Vec<_>>(),
            liquidation_swaps_amount_in: swaps.iter().map(|s| s.amount_in.to()).collect::<Vec<_>>(),
            liquidation_swaps_amount_out: swaps
                .iter()
                .map(|s| s.amount_out.to())
                .collect::<Vec<_>>(),
            liquidations_index: liqs.iter().map(|s| s.trace_index).collect::<Vec<_>>(),
            liquidations_liquidator: liqs.iter().map(|s| s.liquidator).collect::<Vec<_>>(),
            liquidations_liquidatee: liqs.iter().map(|s| s.debtor).collect::<Vec<_>>(),
            liquidations_tokens: liqs
                .iter()
                .map(|s| s.collateral_asset) // TODO: is this supposed
                // to be the collateral or
                // the debt asset?
                .collect::<Vec<_>>(),
            liquidations_amounts: liqs.iter().map(|s| s.amount.to()).collect::<Vec<_>>(),
            liquidations_rewards: todo!(),
            gas_details: gas_details.clone(),
        };

        Some((mev, Box::new(new_liquidation)))
    }
}
