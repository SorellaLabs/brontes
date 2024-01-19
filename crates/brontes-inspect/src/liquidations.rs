use std::{collections::HashSet, sync::Arc};

use brontes_database::{Metadata, Pair};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{ClassifiedMev, Liquidation, MevType, SpecificMev},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    tree::{BlockTree, GasDetails, Node, Root},
    ToFloatNearest,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{b256, Address, B256};

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
                node.data.is_liquidation() || node.data.is_swap(),
                node.subactions
                    .iter()
                    .any(|action| action.is_liquidation() || action.is_swap()),
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
            .cloned()
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
            .cloned()
            .collect::<Vec<_>>();

        let liq_tokens = liqs
            .iter()
            .flat_map(|liq| vec![liq.debt_asset, liq.collateral_asset])
            .collect::<HashSet<_>>();

        let swaps = swaps
            .into_iter()
            .filter(|swap| {
                liq_tokens.contains(&swap.token_out) || liq_tokens.contains(&swap.token_in)
            })
            .collect::<Vec<_>>();

        // TODO: check this
        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx, todo!(), metadata.clone(), true)?;
        let mev_profit_collector = self.inner.profit_collectors(&addr_usd_deltas);

        let gas_finalized = metadata.get_gas_price_usd(gas_details.gas_paid());

        let mev = ClassifiedMev {
            block_number: metadata.block_num,
            eoa,
            tx_hash,
            mev_contract,
            mev_profit_collector,
            finalized_profit_usd: todo!(),
            finalized_bribe_usd: gas_finalized.to_float(),
            mev_type: MevType::Liquidation,
        };

        // TODO: filter swaps not related to liqs?
        let new_liquidation = Liquidation {
            liquidation_tx_hash: tx_hash,
            trigger:             b256!(),
            liquidation_swaps:   swaps,
            liquidations:        liqs,
            gas_details:         gas_details.clone(),
        };

        Some((mev, Box::new(new_liquidation)))
    }
}
