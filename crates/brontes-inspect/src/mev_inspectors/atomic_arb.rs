use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    constants::{get_stable_type, is_euro_stable, is_gold_stable, is_usd_stable, StableType},
    db::dex::PriceAt,
    mev::{AtomicArb, AtomicArbType, Bundle, MevType},
    normalized_actions::{Actions, NormalizedSwap, NormalizedTransfer},
    tree::BlockTree,
    ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::{Either, Itertools};
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;

use crate::{shared_utils::SharedInspectorUtils, BundleData, Inspector, Metadata};

pub struct AtomicArbInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> AtomicArbInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for AtomicArbInspector<'_, DB> {
    type Result = Vec<Bundle>;

    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self::Result {
        let interesting_state = tree.collect_all(
            TreeSearchBuilder::default().with_actions([Actions::is_flash_loan, Actions::is_swap]),
        );

        interesting_state
            .into_par_iter()
            .filter_map(|(tx, actions)| {
                let info = tree.get_tx_info(tx, self.utils.db)?;

                self.process_swaps(info, meta_data.clone(), actions)
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> AtomicArbInspector<'_, DB> {
    fn process_swaps(
        &self,
        info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: Vec<Actions>,
    ) -> Option<Bundle> {
        let (swaps, transfers): (Vec<NormalizedSwap>, Vec<NormalizedTransfer>) = searcher_actions
            .iter()
            .flat_map(|action| match action {
                Actions::Swap(s) => vec![Either::Left(s.clone())],
                Actions::Transfer(t) => vec![Either::Right(t.clone())],
                Actions::FlashLoan(f) => f
                    .child_actions
                    .iter()
                    .flat_map(|a| match a {
                        Actions::Swap(s) => vec![Either::Left(s.clone())],
                        Actions::Transfer(t) => vec![Either::Right(t.clone())],
                        _ => vec![],
                    })
                    .collect(),
                _ => vec![],
            })
            .partition_map(|either| either);

        let possible_arb_type = self.is_possible_arb(&swaps)?;

        let actions = searcher_actions.clone();

        let profit = match possible_arb_type {
            AtomicArbType::Triangle => {
                self.process_triangle_arb(&info, metadata.clone(), &[actions], &swaps)
            }
            AtomicArbType::CrossPair(jump_index) => {
                self.process_cross_pair_arb(&info, metadata.clone(), &swaps, &[actions], jump_index)
            }
            AtomicArbType::StablecoinArb => {
                todo!()
            }
        }?;

        let header = self.utils.build_bundle_header(
            &info,
            profit.to_float(),
            PriceAt::Average,
            &transfers,
            &[info.gas_details],
            metadata,
            MevType::AtomicArb,
        );

        let backrun = AtomicArb {
            tx_hash: info.tx_hash,
            gas_details: info.gas_details,
            swaps,
            arb_type: possible_arb_type,
        };
        let backrun = AtomicArb { tx_hash: info.tx_hash, gas_details: info.gas_details, swaps };

        Some(Bundle { header, data: BundleData::AtomicArb(backrun) })
    }

    fn is_possible_arb(&self, swaps: &[NormalizedSwap]) -> Option<AtomicArbType> {
        match swaps.len() {
            0 | 1 => None,
            2 => {
                // TODO account for stable arbs here
                let start = swaps[0].token_in.address;
                let end = swaps[1].token_out.address;
                let is_triangle = start == end;

                let is_continuous = swaps[0].token_out.address == swaps[1].token_in.address;

                if is_triangle && is_continuous {
                    Some(AtomicArbType::Triangle)
                } else if is_triangle {
                    Some(AtomicArbType::CrossPair(1))
                } else {
                    None
                }
            }
            _ => identify_arb_sequence(swaps),
        }
    }

    // Fix atomic arb to solely work based on swaps & move any transfer related
    // impls to long tail to deal with the scenario in which we have unclassified
    // pools
    fn process_triangle_arb(
        &self,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
        _searcher_actions: &[Vec<Actions>],
        swaps: &[NormalizedSwap],
    ) -> Option<Rational> {
        let rev_usd = self.utils.get_swap_deltas_usd(
            tx_info.tx_index,
            PriceAt::Average,
            swaps,
            metadata.clone(),
        )?;

        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;

        // If the arb is not profitable, check if this is a know searcher or if the tx
        // is private or coinbase.transfers to the builder
        (is_profitable
            || tx_info.is_searcher_of_type(MevType::AtomicArb)
            || tx_info.gas_details.coinbase_transfer.is_some() && tx_info.is_private)
            .then_some(profit)
    }

    fn process_cross_pair_arb(
        &self,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
        swaps: &[NormalizedSwap],
        _searcher_actions: &[Vec<Actions>],
        jump_index: usize,
    ) -> Option<Rational> {
        let is_stable_arb = is_stable_arb(swaps, jump_index);

        let rev_usd = self.utils.get_swap_deltas_usd(
            tx_info.tx_index,
            PriceAt::After,
            swaps,
            metadata.clone(),
        )?;

        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;

        if is_profitable || is_stable_arb {
            Some(rev_usd - gas_used_usd)
        } else {
            // If the arb is not profitable, check if this is a know searcher or if the tx
            // is private or coinbase.transfers to the builder
            (tx_info.is_searcher_of_type(MevType::AtomicArb)
                || tx_info.is_private
                || tx_info.gas_details.coinbase_transfer.is_some())
            .then_some(profit)
        }
    }
}

fn identify_arb_sequence(swaps: &[NormalizedSwap]) -> Option<AtomicArbType> {
    let start_token = &swaps.first().unwrap().token_in.symbol;
    let end_token = &swaps.last().unwrap().token_out.symbol;

    let start_address = &swaps.first().unwrap().token_in.address;
    let end_address = &swaps.last().unwrap().token_out.address;

    if start_address != end_address {
        if is_stable_pair(start_token, end_token) {
            return Some(AtomicArbType::StablecoinArb);
        } else {
            return None;
        }
    }

    let mut last_out = swaps.first().unwrap().token_out.address;

    for (index, swap) in swaps.iter().skip(1).enumerate() {
        if swap.token_in.address != last_out {
            return Some(AtomicArbType::CrossPair(index + 1));
        }
        last_out = swap.token_out.address;
    }

    Some(AtomicArbType::Triangle)
}

fn is_stable_arb(swaps: &[NormalizedSwap], jump_index: usize) -> bool {
    let token_bought = &swaps[jump_index - 1].token_out.symbol;
    let token_sold = &swaps[jump_index].token_in.symbol;

    // Check if this is a stable arb
    is_stable_pair(token_sold, token_bought)
}

fn is_stable_pair(token_in: &str, token_out: &str) -> bool {
    if let Some(stable_type) = get_stable_type(token_in) {
        match stable_type {
            StableType::USD => is_usd_stable(token_out),
            StableType::EURO => is_euro_stable(token_out),
            StableType::GOLD => is_gold_stable(token_out),
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;
    use brontes_types::constants::WETH_ADDRESS;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_backrun() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(hex!("2559813bbb508c4c79e9ccce4703bcb1f149edd7").into())
            .with_expected_profit_usd(0.188588)
            .with_gas_paid_usd(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_false_positive_uni_router() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;
        let tx = hex!("ac1127310fdec0b07e618407eabfb7cdf5ada81dc47e914c76fc759843346a0e").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("c18360217d8f7ab5e7c516566761ea12ce7f9d72").into())
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_false_positive_hex_usdc() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;
        let tx = hex!("e4b8b358118daa26809a1ff77323d825664202c4f31a2afe923f3fe83d7eccc4").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("2b591e99afE9f32eAA6214f7B7629768c40Eeb39").into())
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_triangle_unclassified_pool() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;
        let tx = hex!("707624db0b01bab966c82058d71190031c2bd69098d8efd9c668a89e5acc49ca").into();

        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(WETH_ADDRESS)
            .with_dex_prices()
            .with_expected_profit_usd(2.62)
            .with_gas_paid_usd(10.92);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
