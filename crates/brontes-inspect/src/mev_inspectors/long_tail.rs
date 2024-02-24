use std::{collections::HashMap, sync::Arc};

use alloy_primitives::Address;
use brontes_database::libmdbx::{Libmdbx, LibmdbxReader};
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, BundleHeader},
    normalized_actions::Actions,
    tree::BlockTree,
    TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct LongTailInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> LongTailInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for LongTailInspector<'_, DB> {
    type Result = Vec<Bundle>;

    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Self::Result {
        todo!() /*
                        let interesting_state = tree.collect_all(TreeSearchBuilder::default().with_actions([
                            Actions::is_transfer,
                            Actions::is_flash_loan,
                            Actions::is_swap,
                        ]));

                        interesting_state
                            .into_par_iter()
                            .filter_map(|(tx, actions)| {
                                let info = tree.get_tx_info(tx, self.utils.db)?;

                                self.process_tx(info, meta_data.clone(), actions)
                            })
                            .collect::<Vec<_>>()
                    }
                }

                impl<DB: LibmdbxReader> LongTailInspector<'_, DB> {
                    fn process_tx(
                        &self,
                        info: TxInfo,
                        meta_data: Arc<Metadata>,
                        actions: Vec<Actions>,
                    ) -> Option<Bundle> {
                        todo!()
                    }

                    fn process_long_tail(
                        &self,
                        tx_info: &TxInfo,
                        metadata: Arc<Metadata>,
                        searcher_actions: &[Vec<Actions>],
                    ) -> Option<Rational> {
                        // check the following:
                        // no liquidity collects,
                        // more than 2 transfers or more than 1 swap

                        let collect = searcher_actions.iter().flatten().any(|a| a.is_collect());

                        // if we have a collect and no swaps then return
                        if collect {
                            return None;
                        }

                        let swaps = searcher_actions
                            .iter()
                            .flatten()
                            .map(|a| if a.is_swap() { 1 } else { 0 })
                            .sum::<u64>();
                        let transfers = searcher_actions
                            .iter()
                            .flatten()
                            .map(|a| if a.is_transfer() { 1 } else { 0 })
                            .sum::<u64>();

                        if swaps == 0 || transfers < 3 {
                            return None;
                        }

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
                    }

                    fn get_dex_revenue_usd_with_transfers(
                        &self,
                        idx: u64,
                        at: PriceAt,
                        actions: &[Vec<Actions>],
                        metadata: Arc<Metadata>,
                    ) -> Option<Rational> {
                        /*let mut deltas = HashMap::new();
                        actions
                            .iter()
                            .flatten()
                            .for_each(|action| action.apply_token_deltas(&mut deltas));

                        let deltas = flatten_token_deltas(deltas, actions)?;
                        let addr_usd_deltas =
                            self.utils
                                .usd_delta_by_address(idx as usize, at, &deltas, metadata.clone(), false)?;

                        Some(
                            addr_usd_deltas
                                .values()
                                .fold(Rational::ZERO, |acc, delta| acc + delta),
                        )*/
                        todo!()
                     */
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

//atomically profitable
// (leading zeros could be an indicator but I really doubt they would bother for
// long tail) fresh contract with repeated calls to the same function
// Address has interacted with tornado cash / is funded by tornado cash withdraw
// monero? other privacy bridges
// fixed float deposit addresses
// Check if there are any logs (mev bots shouldn't have any)
// coinbase opcode and transfers
// Selfdestruct opcode
// Any multicalls
// Flashloans yes and repeated calls could be too
// Check if etherscans api to check if bytecode is verified
// The more “f” in the bytecode, the more optimizer run has has been used, hence
// more

// nonce based filtering
