use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    constants::{get_stable_type, is_euro_stable, is_gold_stable, is_usd_stable, StableType},
    db::dex::PriceAt,
    mev::{AtomicArb, Bundle, MevType},
    normalized_actions::{Actions, NormalizedSwap, NormalizedTransfer},
    tree::BlockTree,
    ToFloatNearest, TreeSearchArgs, TxInfo,
};
use itertools::{Either, Itertools};
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;

use crate::{shared_utils::SharedInspectorUtils, BundleData, Inspector, Metadata};

pub struct AtomicArbInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> AtomicArbInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}
//TODO: Add a wrapped asset to unwrapped asset detector to detect wrapped ->
// unwrapped arbs
#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for AtomicArbInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<Bundle> {
        let interesting_state = tree.collect_all(|node| TreeSearchArgs {
            collect_current_node:  node.data.is_swap()
                || node.data.is_transfer()
                || node.data.is_flash_loan(),
            child_node_to_collect: node.get_all_sub_actions().iter().any(|action| {
                action.is_swap() || action.is_transfer() || node.data.is_flash_loan()
            }),
        });

        interesting_state
            .into_par_iter()
            .filter_map(|(tx, actions)| {
                let info = tree.get_tx_info(tx, self.inner.db)?;

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

        let possible_arb_type = self.is_possible_arb(&swaps, &transfers)?;

        let actions = searcher_actions.clone();

        let profit = match possible_arb_type {
            AtomicArbType::LongTail => {
                self.process_long_tail(info, metadata.clone(), &vec![actions])
            }
            AtomicArbType::Triangle => {
                self.process_triangle_arb(info, metadata.clone(), &vec![actions])
            }
            AtomicArbType::CrossPair(jump_index) => self.process_cross_pair_arb(
                info,
                metadata.clone(),
                &swaps,
                &vec![actions],
                jump_index,
            ),
        }?;

        let header = self.inner.build_bundle_header(
            &info,
            profit.to_float(),
            PriceAt::Average,
            &vec![searcher_actions],
            &vec![info.gas_details],
            metadata,
            MevType::AtomicArb,
        );

        let backrun = AtomicArb { tx_hash: info.tx_hash, gas_details: info.gas_details, swaps };

        Some(Bundle { header, data: BundleData::AtomicArb(backrun) })
    }

    fn is_possible_arb(
        &self,
        swaps: &Vec<NormalizedSwap>,
        transfers: &Vec<NormalizedTransfer>,
    ) -> Option<AtomicArbType> {
        match swaps.len() {
            0 | 1 => {
                if transfers.len() >= 2 {
                    Some(AtomicArbType::LongTail)
                } else {
                    None
                }
            }
            2 => {
                let start = swaps[0].token_in.address;
                let end = swaps[1].token_out.address;
                let is_triangle =
                    start == end && swaps[0].token_out.address == swaps[1].token_in.address;
                let is_cross_pair = start == end;

                if is_triangle {
                    Some(AtomicArbType::Triangle)
                } else if is_cross_pair {
                    Some(AtomicArbType::CrossPair(1))
                } else {
                    Some(AtomicArbType::LongTail)
                }
            }
            _ => Some(identify_arb_sequence(&swaps)),
        }
    }

    fn process_triangle_arb(
        &self,
        tx_info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: &Vec<Vec<Actions>>,
    ) -> Option<Rational> {
        let rev_usd = self.inner.get_dex_revenue_usd(
            tx_info.tx_index,
            PriceAt::Average,
            searcher_actions,
            metadata.clone(),
        )?;

        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;

        if is_profitable {
            return Some(rev_usd - gas_used_usd);
        } else {
            // If the arb is not profitable, check if this is a know searcher or if the tx
            // is private or coinbase.transfers to the builder
            match self.inner.db.try_fetch_searcher_info(tx_info.eoa) {
                Ok(Some(_info)) => Some(profit),
                Ok(None) => {
                    if tx_info.gas_details.coinbase_transfer.is_some() && tx_info.is_private {
                        Some(profit)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
    }

    fn process_cross_pair_arb(
        &self,
        tx_info: TxInfo,

        metadata: Arc<Metadata>,
        swaps: &Vec<NormalizedSwap>,
        searcher_actions: &Vec<Vec<Actions>>,
        jump_index: usize,
    ) -> Option<Rational> {
        let is_stable_arb = is_stable_arb(swaps, jump_index);

        let rev_usd = self.inner.get_dex_revenue_usd(
            tx_info.tx_index,
            PriceAt::After,
            searcher_actions,
            metadata.clone(),
        )?;

        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;

        if is_profitable || is_stable_arb {
            return Some(rev_usd - gas_used_usd);
        } else {
            // If the arb is not profitable, check if this is a know searcher or if the tx
            // is private or coinbase.transfers to the builder
            match self.inner.db.try_fetch_searcher_info(tx_info.eoa) {
                Ok(Some(_info)) => Some(profit),
                Ok(None) => {
                    if tx_info.is_private || tx_info.gas_details.coinbase_transfer.is_some() {
                        Some(profit)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
    }

    fn process_long_tail(
        &self,
        tx_info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: &Vec<Vec<Actions>>,
    ) -> Option<Rational> {
        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let rev_usd = self.inner.get_dex_revenue_usd(
            tx_info.tx_index,
            PriceAt::Average,
            searcher_actions,
            metadata.clone(),
        )?;

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;

        if is_profitable {
            match self.inner.db.try_fetch_searcher_info(tx_info.eoa) {
                Ok(Some(info)) => {
                    if info.mev.contains(&MevType::AtomicArb) {
                        Some(profit)
                    } else {
                        None
                    }
                }
                Ok(None) => {
                    if tx_info.is_private
                        && tx_info.gas_details.coinbase_transfer.is_some()
                        && !tx_info.is_verified_contract
                    {
                        Some(profit)
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }
}

fn identify_arb_sequence(swaps: &Vec<NormalizedSwap>) -> AtomicArbType {
    let start_token = swaps.first().unwrap().token_in.address;
    let end_token = swaps.last().unwrap().token_out.address;

    if start_token != end_token {
        return AtomicArbType::LongTail
    }

    let mut last_out = swaps.first().unwrap().token_out.address;

    for (index, swap) in swaps.iter().skip(1).enumerate() {
        if swap.token_in.address != last_out {
            return AtomicArbType::CrossPair(index + 1)
        }
        last_out = swap.token_out.address;
    }

    AtomicArbType::Triangle
}

fn is_stable_arb(swaps: &Vec<NormalizedSwap>, jump_index: usize) -> bool {
    let token_bought = &swaps[jump_index - 1].token_out.symbol;
    let token_sold = &swaps[jump_index].token_in.symbol;

    // Check if this is a stable arb
    if let Some(stable_type) = get_stable_type(&token_bought) {
        match stable_type {
            StableType::USD => is_usd_stable(&token_sold),
            StableType::EURO => is_euro_stable(&token_sold),
            StableType::GOLD => is_gold_stable(&token_sold),
        }
    } else {
        false
    }
}

/// Represents the different types of atomic arb
/// A triangle arb is a simple arb that goes from token A -> B -> C -> A
/// A cross pair arb is a more complex arb that goes from token A -> B -> C -> A
enum AtomicArbType {
    LongTail,
    Triangle,
    CrossPair(usize),
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;
    use brontes_types::constants::USDT_ADDRESS;
    use serial_test::serial;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[tokio::test]
    #[serial]
    async fn test_backrun() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(hex!("2559813bbb508c4c79e9ccce4703bcb1f149edd7").into())
            .with_expected_profit_usd(0.188588)
            .with_gas_paid_usd(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_simple_triangular() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);
        let tx = hex!("67d9884157d495df4eaf24b0d65aeca38e1b5aeb79200d030e3bb4bd2cbdcf88").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("c98835e792553e505ae46e73a6fd27a23985acca").into())
            .with_dex_prices()
            .with_expected_profit_usd(311.18)
            .with_gas_paid_usd(91.51);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_not_false_positive_uni_router() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);
        let tx = hex!("ac1127310fdec0b07e618407eabfb7cdf5ada81dc47e914c76fc759843346a0e").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_not_false_positive_hex_usdc() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5);
        let tx = hex!("e4b8b358118daa26809a1ff77323d825664202c4f31a2afe923f3fe83d7eccc4").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    //TODO:
    #[tokio::test]
    #[serial]
    async fn test_cross_stable_arb() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5);
        let tx = hex!("397c98efa1991e0384db16c56bd1693fb82addc7d932328941912afa8176cdb1").into();
        let config = InspectorTxRunConfig::new(Inspectors::AtomicArb)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices();

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}

// TODO: Debug: https://etherscan.io/tx/0xaf410a70a5b693225555f30c44d62eaed265d04ec49a00409fe2aaa61ea5a881
