use std::{collections::HashMap, sync::Arc};

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

use crate::{
    mev_inspectors::shared_utils::ActionRevenueCalculation, shared_utils::SharedInspectorUtils,
    BundleData, Inspector, Metadata,
};

pub struct AtomicArbInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> AtomicArbInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self {
            inner: SharedInspectorUtils::new(quote, db),
        }
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
        let interesting_state = tree.collect_all(|node, info| TreeSearchArgs {
            collect_current_node: info
                .get_ref(node.data)
                .map(|action| action.is_transfer() || action.is_flash_loan() || action.is_swap())
                .unwrap_or_default(),
            child_node_to_collect: node
                .subactions
                .iter()
                .filter_map(|node| info.get_ref(*node))
                .any(|action| action.is_transfer() || action.is_flash_loan() || action.is_swap()),
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
            AtomicArbType::LongTail => self.process_long_tail(&info, metadata.clone(), &[actions]),
            AtomicArbType::Triangle => {
                self.process_triangle_arb(&info, metadata.clone(), &[actions])
            }
            AtomicArbType::CrossPair(jump_index) => {
                self.process_cross_pair_arb(&info, metadata.clone(), &swaps, &[actions], jump_index)
            }
        }?;

        let header = self.inner.build_bundle_header(
            &info,
            profit.to_float(),
            PriceAt::Average,
            &[searcher_actions],
            &[info.gas_details],
            metadata,
            MevType::AtomicArb,
        );

        let backrun = AtomicArb {
            tx_hash: info.tx_hash,
            gas_details: info.gas_details,
            swaps,
        };

        Some(Bundle {
            header,
            data: BundleData::AtomicArb(backrun),
        })
    }

    fn is_possible_arb(
        &self,
        swaps: &[NormalizedSwap],
        transfers: &[NormalizedTransfer],
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
            _ => Some(identify_arb_sequence(swaps)),
        }
    }

    fn process_triangle_arb(
        &self,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: &[Vec<Actions>],
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
        searcher_actions: &[Vec<Actions>],
        jump_index: usize,
    ) -> Option<Rational> {
        let is_stable_arb = is_stable_arb(swaps, jump_index);

        let rev_usd = self.get_dex_revenue_usd_with_transfers(
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

    fn process_long_tail(
        &self,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: &[Vec<Actions>],
    ) -> Option<Rational> {
        let gas_used = tx_info.gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let rev_usd = self.get_dex_revenue_usd_with_transfers(
            tx_info.tx_index,
            PriceAt::Lowest,
            searcher_actions,
            metadata.clone(),
        )?;

        let profit = &rev_usd - &gas_used_usd;

        let is_profitable = profit > Rational::ZERO;
        is_profitable.then_some(profit)

        // is_profitable
        //     .then(
        //         || match self.inner.db.try_fetch_searcher_info(tx_info.eoa) {
        //             Ok(info) => info.mev.contains(&MevType::AtomicArb).then_some(profit),
        //             Err(_) => (tx_info.is_private
        //                 && tx_info.gas_details.coinbase_transfer.is_some()
        //                 && !tx_info.is_verified_contract)
        //                 .then_some(profit),
        //         },
        //     )
        //     .flatten()
    }

    fn get_dex_revenue_usd_with_transfers(
        &self,
        idx: u64,
        at: PriceAt,
        actions: &[Vec<Actions>],
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let mut deltas = HashMap::new();
        actions
            .iter()
            .flatten()
            .for_each(|action| action.apply_token_deltas(&mut deltas));

        let deltas = flatten_token_deltas(deltas, actions)?;
        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx as usize, at, &deltas, metadata.clone(), false)?;

        Some(
            addr_usd_deltas
                .values()
                .fold(Rational::ZERO, |acc, delta| acc + delta),
        )
    }
}

fn identify_arb_sequence(swaps: &[NormalizedSwap]) -> AtomicArbType {
    let start_token = swaps.first().unwrap().token_in.address;
    let end_token = swaps.last().unwrap().token_out.address;

    if start_token != end_token {
        return AtomicArbType::LongTail;
    }

    let mut last_out = swaps.first().unwrap().token_out.address;

    for (index, swap) in swaps.iter().skip(1).enumerate() {
        if swap.token_in.address != last_out {
            return AtomicArbType::CrossPair(index + 1);
        }
        last_out = swap.token_out.address;
    }

    AtomicArbType::Triangle
}

fn is_stable_arb(swaps: &[NormalizedSwap], jump_index: usize) -> bool {
    let token_bought = &swaps[jump_index - 1].token_out.symbol;
    let token_sold = &swaps[jump_index].token_in.symbol;

    // Check if this is a stable arb
    if let Some(stable_type) = get_stable_type(token_bought) {
        match stable_type {
            StableType::USD => is_usd_stable(token_sold),
            StableType::EURO => is_euro_stable(token_sold),
            StableType::GOLD => is_gold_stable(token_sold),
        }
    } else {
        false
    }
}
type TokenDeltasCalc = HashMap<Address, HashMap<Address, HashMap<Address, Rational>>>;
type TokenDeltas = HashMap<Address, HashMap<Address, Rational>>;

// if theres any address with a single non-zero token delta. Then
// that is the person with the result delta and we just use that.
// otherwise, if 2 transfers, last transfer to, else same first last
// this also assumes that a arber doesn't dust any contracts
fn flatten_token_deltas(deltas: TokenDeltasCalc, actions: &[Vec<Actions>]) -> Option<TokenDeltas> {
    let mut deltas = deltas
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                v.into_iter()
                    .map(|(k, v)| (k, v.into_values().sum::<Rational>()))
                    .filter(|(_, v)| v.ne(&Rational::ZERO))
                    .into_grouping_map()
                    .sum(),
            )
        })
        .filter(|(_, v)| !v.is_empty())
        .collect::<HashMap<_, HashMap<_, _>>>();

    // if there is a address with a single token delta, then it
    // is the proper pool.
    if deltas.iter().any(|(_, v)| v.len() == 1) {
        deltas.retain(|_, v| v.len() == 1);
        return Some(deltas);
    }

    let transfers = actions
        .iter()
        .flatten()
        .filter(|t| t.is_transfer())
        .map(|t| t.clone().force_transfer())
        .sorted_by(|a, b| a.trace_index.cmp(&b.trace_index))
        .collect_vec();

    // if just two transfers, result person will always be last,
    match transfers.len() {
        0 | 1 => None,
        2 => {
            let final_address = transfers.last().unwrap().to;
            deltas.retain(|k, _| *k == final_address);
            Some(deltas)
        }
        _ => {
            // grab first and last transfers
            let first = transfers.first().unwrap();
            let last = transfers.last().unwrap();
            if first.to == last.from {
                deltas.retain(|k, _| *k == first.to);
                Some(deltas)
            } else if first.from == last.to {
                deltas.retain(|k, _| *k == first.from);
                Some(deltas)
            } else {
                tracing::error!("shouldn't be hit");
                None
            }
        }
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
