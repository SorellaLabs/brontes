use std::{collections::hash_map::Entry, sync::Arc};

use alloy_primitives::{Address, B256};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::{OutlierMetrics, ProfitMetrics};
use brontes_types::{
    collect_address_set_for_accounting,
    db::dex::PriceAt,
    mev::{Bundle, JitLiquidity, MevType},
    normalized_actions::{
        accounting::ActionAccounting, NormalizedBurn, NormalizedCollect, NormalizedMint,
    },
    ActionIter, BlockData, FastHashMap, FastHashSet, GasDetails, MultiBlockData, ToFloatNearest,
    TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::TxHash;

use super::types::{PossibleJit, PossibleJitWithInfo};
use crate::{
    shared_utils::SharedInspectorUtils, Action, BlockTree, BundleData, Inspector, Metadata,
    MAX_PROFIT, MIN_PROFIT,
};

pub struct JitInspector<'db, DB: LibmdbxReader> {
    pub utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> JitInspector<'db, DB> {
    pub fn new(
        quote: Address,
        db: &'db DB,
        metrics: Option<OutlierMetrics>,
        profit_metrics: Option<ProfitMetrics>,
    ) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db, metrics, profit_metrics) }
    }
}

impl<DB: LibmdbxReader> Inspector for JitInspector<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "Jit"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn inspect_block(&self, data: MultiBlockData) -> Self::Result {
        let BlockData { metadata, tree } = data.get_most_recent_block();

        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::Jit, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree.clone(), metadata.clone()))
    }
}

impl<DB: LibmdbxReader> JitInspector<'_, DB> {
    pub fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        self.possible_jit_set(tree.clone())
            .into_iter()
            .filter_map(
                |PossibleJitWithInfo {
                     inner:
                         PossibleJit { frontrun_txes, backrun_tx, executor_contract, victims, .. },
                     victim_info,
                     backrun,
                     front_runs,
                 }| {
                    let searcher_actions = self.get_searcher_actions(
                        frontrun_txes.iter().chain([backrun_tx].iter()),
                        tree.clone(),
                    );

                    tracing::trace!(?frontrun_txes, ?backrun_tx, "checking if jit");

                    if searcher_actions.is_empty() {
                        tracing::trace!("no searcher actions found");
                        return None
                    }

                    let victim_actions =
                        self.get_victim_actions(victims, tree.clone(), executor_contract)?;

                    self.calculate_jit(
                        front_runs,
                        backrun,
                        metadata.clone(),
                        searcher_actions,
                        victim_actions,
                        victim_info,
                        0,
                    )
                },
            )
            .flatten()
            .collect::<Vec<_>>()
    }

    fn get_searcher_actions<'a>(
        &self,
        i: impl Iterator<Item = &'a TxHash>,
        tree: Arc<BlockTree<Action>>,
    ) -> Vec<Vec<Action>> {
        i.map(|tx| {
            self.utils
                .flatten_nested_actions(
                    tree.clone().collect(
                        tx,
                        TreeSearchBuilder::default().with_actions([
                            Action::is_mint,
                            Action::is_burn,
                            Action::is_transfer,
                            Action::is_eth_transfer,
                            Action::is_nested_action,
                        ]),
                    ),
                    &|actions| {
                        actions.is_mint()
                            || actions.is_burn()
                            || actions.is_collect()
                            || actions.is_transfer()
                            || actions.is_eth_transfer()
                    },
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<Vec<Action>>>()
    }

    fn calculate_recursive(
        frontrun_info: &[TxInfo],
        backrun_info: &TxInfo,
        searcher_actions: &[Vec<Action>],
    ) -> Option<bool> {
        let front_is_mint_back_is_burn = searcher_actions.last()?.iter().any(|h| h.is_burn())
            || searcher_actions
                .iter()
                .take(searcher_actions.len() - 1)
                .all(|h| h.iter().any(|a| a.is_mint()));

        let matching_eoas = frontrun_info.first()?.eoa == backrun_info.eoa;
        // ensure tokens match
        let f = searcher_actions.first()?;
        let Some(Action::Mint(mint)) = f.iter().find(|f| f.is_mint()) else { return Some(true) };
        let l = searcher_actions.last()?;
        let Some(Action::Burn(burn)) = l.iter().find(|f| f.is_burn()) else { return Some(true) };
        let mint_burn_eq = mint.token.iter().all(|mt| burn.token.contains(mt));

        Some(!front_is_mint_back_is_burn || !matching_eoas || !mint_burn_eq)
    }

    fn calculate_jit(
        &self,
        frontrun_info: Vec<TxInfo>,
        backrun_info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: Vec<Vec<Action>>,
        // victim
        victim_actions: Vec<Vec<Action>>,
        victim_info: Vec<Vec<TxInfo>>,
        recursive: u8,
    ) -> Option<Vec<Bundle>> {
        if Self::calculate_recursive(&frontrun_info, &backrun_info, &searcher_actions)? {
            tracing::trace!("recusing time");
            return self.recursive_possible_jits(
                frontrun_info,
                backrun_info,
                metadata,
                searcher_actions,
                victim_actions,
                victim_info,
                recursive,
            )
        }
        tracing::trace!("formulating");

        // grab all mints and burns
        let ((mints, burns, collect), rem): ((Vec<_>, Vec<_>, Vec<_>), Vec<_>) = searcher_actions
            .clone()
            .into_iter()
            .flatten()
            .action_split_out((Action::try_mint, Action::try_burn, Action::try_collect));

        if mints.is_empty() || (burns.is_empty() && collect.is_empty()) {
            tracing::trace!("missing mints & burns");
            return None
        }
        self.ensure_valid_structure(&mints, &burns, &victim_actions)?;

        let mut info_set = frontrun_info.clone();
        info_set.push(backrun_info.clone());

        let mev_addresses: FastHashSet<Address> = collect_address_set_for_accounting(&info_set);

        let deltas = rem
            .into_iter()
            .filter(|f| f.is_transfer() || f.is_eth_transfer())
            .chain(
                info_set
                    .iter()
                    .flat_map(|info| info.get_total_eth_value())
                    .cloned()
                    .map(Action::from),
            )
            .account_for_actions();

        let (rev, mut has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
            info_set.last()?.tx_index,
            PriceAt::After,
            &mev_addresses,
            &deltas,
            metadata.clone(),
            true,
        ) {
            (Some(rev), true)
        } else {
            (Some(Rational::ZERO), false)
        };

        let (hashes, gas_details): (Vec<_>, Vec<_>) = info_set
            .iter()
            .map(|info| info.clone().split_to_storage_info())
            .unzip();

        let (victim_hashes, victim_gas_details): (Vec<_>, Vec<_>) = victim_info
            .into_iter()
            .flatten()
            .map(|info| info.split_to_storage_info())
            .unzip();

        let bribe = self.get_bribes(metadata.clone(), &gas_details);
        let mut profit = rev
            .map(|rev| rev - &bribe)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        if profit >= MAX_PROFIT || profit <= MIN_PROFIT {
            has_dex_price = false;
            profit = Rational::ZERO;
        }

        let mut bundle_hashes = Vec::new();
        bundle_hashes.push(hashes[0]);
        bundle_hashes.extend(victim_hashes.clone());
        bundle_hashes.push(hashes[1]);

        let header = self.utils.build_bundle_header(
            vec![deltas],
            bundle_hashes,
            info_set.last()?,
            profit.to_float(),
            &gas_details,
            metadata.clone(),
            MevType::Jit,
            !has_dex_price,
            |this, token, amount| {
                this.get_token_value_dex(
                    info_set.last()?.tx_index as usize,
                    PriceAt::Average,
                    token,
                    &amount,
                    &metadata,
                )
            },
        );

        let jit_details = self.build_jit_type(
            hashes,
            gas_details,
            metadata.block_num,
            mints,
            burns,
            collect,
            victim_hashes,
            victim_gas_details,
            &victim_actions,
        )?;

        Some(vec![Bundle { header, data: BundleData::Jit(jit_details) }])
    }

    fn build_jit_type(
        &self,
        mut hashes: Vec<TxHash>,
        mut gas_details: Vec<GasDetails>,
        block_number: u64,
        mints: Vec<NormalizedMint>,
        burns: Vec<NormalizedBurn>,
        collect: Vec<NormalizedCollect>,
        victim_hashes: Vec<TxHash>,
        victim_gas_details: Vec<GasDetails>,
        victim_actions: &[Vec<Action>],
    ) -> Option<JitLiquidity> {
        let victim_swaps = victim_actions
            .iter()
            .map(|tx_actions| {
                tx_actions
                    .iter()
                    .filter(|action| action.is_swap())
                    .map(|f| f.clone().force_swap())
                    .collect::<Vec<_>>()
            })
            .collect();

        Some(JitLiquidity {
            block_number,
            frontrun_mint_tx_hash: hashes[0],
            frontrun_mint_gas_details: gas_details[0],
            frontrun_mints: mints,
            victim_swaps_tx_hashes: victim_hashes.clone(),
            victim_swaps,
            victim_swaps_gas_details_tx_hashes: victim_hashes,
            victim_swaps_gas_details: victim_gas_details,
            backrun_burn_tx_hash: hashes.pop()?,
            backrun_burn_gas_details: gas_details.pop()?,
            backrun_burns: Some(collect)
                .filter(|f| !f.is_empty())
                .map(|collect| {
                    collect
                        .into_iter()
                        .map(|c| NormalizedBurn {
                            recipient:   c.recipient,
                            trace_index: c.trace_index,
                            protocol:    c.protocol,
                            amount:      c.amount,
                            token:       c.token,
                            pool:        c.pool,
                            from:        c.from,
                        })
                        .collect_vec()
                })
                .unwrap_or(burns),
        })
    }

    fn ensure_valid_structure(
        &self,
        mints: &[NormalizedMint],
        burns: &[NormalizedBurn],
        victim_actions: &[Vec<Action>],
    ) -> Option<()> {
        // assert mints and burns are same pool
        let mut pools = FastHashSet::default();
        mints.iter().for_each(|m| {
            pools.insert(m.pool);
        });

        if !burns.iter().any(|b| pools.contains(&b.pool)) {
            tracing::trace!("no burn overlaps");
            return None
        }

        // ensure we have overlap
        let v_swaps = victim_actions
            .iter()
            .flatten()
            .filter(|a| a.is_swap())
            .map(|a| a.clone().force_swap())
            .collect::<Vec<_>>();

        (v_swaps
            .into_iter()
            .map(|swap| pools.contains(&swap.pool) as usize)
            .sum::<usize>()
            != 0)
            .then_some(())
    }

    fn recursive_possible_jits(
        &self,
        frontrun_info: Vec<TxInfo>,
        backrun_info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: Vec<Vec<Action>>,
        // victim
        victim_actions: Vec<Vec<Action>>,
        victim_info: Vec<Vec<TxInfo>>,
        mut recursive: u8,
    ) -> Option<Vec<Bundle>> {
        let mut res = vec![];

        if recursive >= 10 {
            return None
        }
        if frontrun_info.len() > 1 {
            recursive += 1;
            // remove dropped sandwiches
            if victim_info.is_empty() || victim_actions.is_empty() {
                return None
            }

            let back_shrink = {
                let mut victim_info = victim_info.to_vec();
                let mut victim_actions = victim_actions.to_vec();
                let mut front_run_info = frontrun_info.to_vec();
                victim_info.pop()?;
                victim_actions.pop()?;
                // remove last searcher action
                let mut searcher_actions = searcher_actions.clone();
                searcher_actions.pop()?;
                let backrun_info = front_run_info.pop()?;

                if victim_actions.iter().flatten().count() == 0 {
                    return None
                }

                self.calculate_jit(
                    front_run_info,
                    backrun_info,
                    metadata.clone(),
                    searcher_actions,
                    victim_actions,
                    victim_info,
                    recursive,
                )
            };

            let front_shrink = {
                let mut victim_info = victim_info.to_vec();
                let mut victim_actions = victim_actions.to_vec();
                let mut possible_front_runs_info = frontrun_info.to_vec();
                let mut searcher_actions = searcher_actions.to_vec();
                // ensure we don't loose the last tx

                victim_info.remove(0);
                victim_actions.remove(0);
                possible_front_runs_info.remove(0);
                searcher_actions.remove(0);

                if victim_actions.iter().flatten().count() == 0 {
                    return None
                }

                self.calculate_jit(
                    possible_front_runs_info,
                    backrun_info,
                    metadata.clone(),
                    searcher_actions,
                    victim_actions,
                    victim_info,
                    recursive,
                )
            };
            if let Some(front) = front_shrink {
                res.extend(front);
            }
            if let Some(back) = back_shrink {
                res.extend(back);
            }
            return Some(res)
        }

        None
    }

    fn possible_jit_set(&self, tree: Arc<BlockTree<Action>>) -> Vec<PossibleJitWithInfo> {
        let iter = tree.tx_roots.iter();

        if iter.len() < 3 {
            return vec![]
        }

        let mut set: FastHashMap<Address, PossibleJit> = FastHashMap::default();
        let mut duplicate_mev_contracts: FastHashMap<Address, (B256, Address)> =
            FastHashMap::default();

        let mut duplicate_senders: FastHashMap<Address, B256> = FastHashMap::default();
        let mut possible_victims: FastHashMap<B256, Vec<B256>> = FastHashMap::default();

        for root in iter {
            if root.get_root_action().is_revert() {
                continue
            }

            match duplicate_mev_contracts.entry(root.get_to_address()) {
                // If this contract has not been called within this block, we insert the tx hash
                // into the map
                Entry::Vacant(duplicate_mev_contract) => {
                    duplicate_mev_contract.insert((root.tx_hash, root.head.address));
                }
                Entry::Occupied(mut o) => {
                    // Get's prev tx hash &  for this sender & replaces it with the current tx hash
                    let (prev_tx_hash, frontrun_eoa) = o.get_mut();

                    if let Some(frontrun_victims) = possible_victims.remove(prev_tx_hash) {
                        match set.entry(root.get_to_address()) {
                            Entry::Vacant(e) => {
                                e.insert(PossibleJit {
                                    eoa:               *frontrun_eoa,
                                    frontrun_txes:     vec![*prev_tx_hash],
                                    backrun_tx:        root.tx_hash,
                                    executor_contract: root.get_to_address(),
                                    victims:           vec![frontrun_victims],
                                });
                            }
                            Entry::Occupied(mut o) => {
                                let sandwich = o.get_mut();
                                sandwich.frontrun_txes.push(*prev_tx_hash);
                                sandwich.backrun_tx = root.tx_hash;
                                sandwich.victims.push(frontrun_victims);
                            }
                        }
                    }

                    *prev_tx_hash = root.tx_hash;
                }
            }

            match duplicate_senders.entry(root.head.address) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(root.tx_hash);
                }
                Entry::Occupied(mut o) => {
                    // Get's prev tx hash for this sender & replaces it with the current tx hash
                    let prev_tx_hash = o.insert(root.tx_hash);
                    if let Some(frontrun_victims) = possible_victims.remove(&prev_tx_hash) {
                        match set.entry(root.head.address) {
                            Entry::Vacant(e) => {
                                e.insert(PossibleJit {
                                    eoa:               root.head.address,
                                    frontrun_txes:     vec![prev_tx_hash],
                                    backrun_tx:        root.tx_hash,
                                    executor_contract: root.get_to_address(),
                                    victims:           vec![frontrun_victims],
                                });
                            }
                            Entry::Occupied(mut o) => {
                                let sandwich = o.get_mut();
                                sandwich.frontrun_txes.push(prev_tx_hash);
                                sandwich.backrun_tx = root.tx_hash;
                                sandwich.victims.push(frontrun_victims);
                            }
                        }
                    }

                    // Add current transaction hash to the list of transactions for this sender
                    o.insert(root.tx_hash);
                }
            }

            // Now, for each existing entry in possible_victims, we add the current
            // transaction hash as a potential victim, if it is not the same as
            // the key (which represents another transaction hash)
            for v in possible_victims.values_mut() {
                v.push(root.tx_hash);
            }

            possible_victims.insert(root.tx_hash, vec![]);
        }

        let set = Itertools::unique(set.into_values())
            .flat_map(Self::partition_into_gaps)
            .collect::<Vec<_>>();

        // split out
        let tx_set = set
            .iter()
            .filter_map(|jit| {
                if jit.victims.len() > 10 {
                    return None
                }

                let mut set = vec![jit.backrun_tx];
                set.extend(jit.victims.iter().flatten().cloned());
                set.extend(jit.frontrun_txes.clone());
                if !(set
                    .iter()
                    .any(|tx| tree.tx_must_contain_action(*tx, |a| a.is_mint()).unwrap())
                    && set
                        .iter()
                        .any(|tx| tree.tx_must_contain_action(*tx, |a| a.is_burn()).unwrap()))
                {
                    return None
                }
                Some(set)
            })
            .flatten()
            .unique()
            .collect::<Vec<_>>();

        let tx_info_map = tree
            .get_tx_info_batch(&tx_set, self.utils.db)
            .into_iter()
            .flatten()
            .map(|info| (info.tx_hash, info))
            .collect::<FastHashMap<_, _>>();

        set.into_iter()
            .filter(|jit| {
                jit.victims.iter().flatten().count() <= 20
                    && !jit.frontrun_txes.is_empty()
                    && !jit.victims.is_empty()
            })
            .filter_map(|jit| PossibleJitWithInfo::from_jit(jit, &tx_info_map))
            .collect_vec()
    }

    fn get_bribes(&self, price: Arc<Metadata>, gas: &[GasDetails]) -> Rational {
        let bribe = gas.iter().map(|gas| gas.gas_paid()).sum::<u128>();

        price.get_gas_price_usd(bribe, self.utils.quote)
    }

    fn partition_into_gaps(ps: PossibleJit) -> Vec<PossibleJit> {
        let PossibleJit { eoa, frontrun_txes, backrun_tx, executor_contract, victims } = ps;
        let mut results = vec![];
        let mut victim_sets = vec![];
        let mut last_partition = 0;

        victims.into_iter().enumerate().for_each(|(i, group_set)| {
            if group_set.is_empty() {
                results.push(PossibleJit {
                    eoa,
                    executor_contract,
                    victims: std::mem::take(&mut victim_sets),
                    frontrun_txes: frontrun_txes[last_partition..i].to_vec(),
                    backrun_tx: frontrun_txes.get(i).copied().unwrap_or(backrun_tx),
                });
                last_partition = i + 1;
            } else {
                victim_sets.push(group_set);
            }
        });

        if results.is_empty() {
            results.push(PossibleJit {
                eoa,
                executor_contract,
                victims: victim_sets,
                frontrun_txes,
                backrun_tx,
            });
        } else if !victim_sets.is_empty() {
            // add remainder
            results.push(PossibleJit {
                eoa,
                executor_contract,
                victims: victim_sets,
                frontrun_txes: frontrun_txes[last_partition..].to_vec(),
                backrun_tx,
            });
        }

        results
    }

    fn get_victim_actions(
        &self,
        victims: Vec<Vec<TxHash>>,
        tree: Arc<BlockTree<Action>>,
        executor_contract: Address,
    ) -> Option<Vec<Vec<Action>>> {
        let victim_actions = victims
            .iter()
            .flatten()
            .map(|victim| {
                self.utils
                    .flatten_nested_actions(
                        tree.clone().collect(
                            victim,
                            TreeSearchBuilder::default()
                                .with_actions([Action::is_swap, Action::is_nested_action]),
                        ),
                        &|actions| actions.is_swap(),
                    )
                    .collect::<Vec<_>>()
            })
            .collect_vec();

        Some(victim_actions).filter(|_| {
            !victims
                .iter()
                .flatten()
                .map(|v| tree.get_root(*v).unwrap().get_root_action())
                .filter(|d| !d.is_revert())
                .any(|d| executor_contract == d.get_to_address())
        })
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
    async fn test_jit() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .with_block(18539312)
            .needs_tokens(vec![
                hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").into(),
                hex!("b17548c7b510427baac4e267bea62e800b247173").into(),
                hex!("ed4e879087ebd0e8a77d66870012b5e0dffd0fa4").into(),
                hex!("50d1c9771902476076ecfc8b2a83ad6b9355a4c9").into(),
            ])
            .with_gas_paid_usd(90.875025)
            .with_expected_profit_usd(13.58);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_only_jit() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![
                hex!("95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce").into(),
                WETH_ADDRESS,
            ])
            .with_block(18521071)
            .with_gas_paid_usd(92.65)
            .with_expected_profit_usd(26.48);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_jit_blur_double() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![
                hex!("95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce").into(),
                WETH_ADDRESS,
            ])
            .with_mev_tx_hashes(vec![
                hex!("70a315ed0b31138a0b841d9760dc6d4595414e50fecb60f05e031880f0d9398f").into(),
                hex!("590edbb9e1046405a2a3586208e1e9384b8eca93dcbf03e9216da53ca8f94a6d").into(),
                hex!("ab001a0981e3da3d057c1b0c939a988d4d7cc98a903c66699feb59fc028ffe77").into(),
                hex!("b420b67fab4f1902bcd1284934d9610631b9da9e616780dbcc85d7c815b50896").into(),
            ])
            .with_gas_paid_usd(81.8)
            .with_expected_profit_usd(-92.53);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_old_v3_jit_721() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(16862007)
            .with_gas_paid_usd(40.7)
            .with_expected_profit_usd(-25.62);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_multihop_jit() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 10.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(18884329)
            .with_gas_paid_usd(792.89)
            .with_expected_profit_usd(17.9);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_misclassified_jit() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(16637669);

        test_utils.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_misclassified_jit2() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(19506666);
        test_utils.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    pub async fn test_jit_sandwich_multi_hop_jit() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.2).await;

        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(18674873)
            .with_gas_paid_usd(273.9)
            .with_expected_profit_usd(18.1)
            .with_dex_prices();

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
