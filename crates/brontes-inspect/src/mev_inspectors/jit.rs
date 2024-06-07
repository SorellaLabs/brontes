use std::{collections::hash_map::Entry, sync::Arc};

use alloy_primitives::{Address, B256};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    collect_address_set_for_accounting,
    db::dex::PriceAt,
    mev::{Bundle, JitLiquidity, MevType},
    normalized_actions::accounting::ActionAccounting,
    ActionIter, FastHashMap, FastHashSet, GasDetails, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};

use crate::{
    shared_utils::SharedInspectorUtils, Action, BlockTree, BundleData, Inspector, Metadata,
};

#[derive(Debug)]
struct PossibleJitWithInfo {
    pub front_runs:  Vec<TxInfo>,
    pub backrun:     TxInfo,
    pub victim_info: Vec<Vec<TxInfo>>,
    pub inner:       PossibleJit,
}
impl PossibleJitWithInfo {
    pub fn from_jit(ps: PossibleJit, info_set: &FastHashMap<B256, TxInfo>) -> Option<Self> {
        let backrun = info_set.get(&ps.backrun_tx).cloned()?;
        let mut frontruns = vec![];

        for fr in &ps.frontrun_txes {
            frontruns.push(info_set.get(fr).cloned()?);
        }

        let mut victims = vec![];
        for victim in &ps.victims {
            let mut set = vec![];
            for v in victim {
                set.push(info_set.get(v).cloned()?);
            }
            victims.push(set);
        }

        Some(PossibleJitWithInfo {
            front_runs: frontruns,
            backrun,
            victim_info: victims,
            inner: ps,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct PossibleJit {
    pub eoa:               Address,
    pub frontrun_txes:     Vec<B256>,
    pub backrun_tx:        B256,
    pub executor_contract: Address,
    pub victims:           Vec<Vec<B256>>,
}

pub struct JitInspector<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> JitInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB, metrics: Option<OutlierMetrics>) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db, metrics) }
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

    fn inspect_block(&self, tree: Arc<BlockTree<Action>>, metadata: Arc<Metadata>) -> Self::Result {
        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::Jit, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree, metadata))
    }
}

impl<DB: LibmdbxReader> JitInspector<'_, DB> {
    fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        self.possible_jit_set(tree.clone())
            .into_iter()
            .map(|f| {
                tracing::info!("{:#?}", f);
                f
            })
            .filter_map(
                |PossibleJitWithInfo {
                     inner:
                         PossibleJit { frontrun_txes, backrun_tx, executor_contract, victims, .. },
                     victim_info,
                     backrun,
                     front_runs,
                 }| {
                    let searcher_actions = frontrun_txes
                        .iter()
                        .chain([backrun_tx].iter())
                        .map(|tx| {
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
                        .collect::<Vec<Vec<Action>>>();

                    tracing::trace!(?frontrun_txes, ?backrun_tx, "checking if jit");

                    if searcher_actions.is_empty() {
                        tracing::trace!("no searcher actions found");
                        return None
                    }

                    let victim_actions = victims
                        .iter()
                        .flatten()
                        .map(|victim| {
                            self.utils
                                .flatten_nested_actions(
                                    tree.clone().collect(
                                        victim,
                                        TreeSearchBuilder::default().with_actions([
                                            Action::is_swap,
                                            Action::is_nested_action,
                                        ]),
                                    ),
                                    &|actions| actions.is_swap(),
                                )
                                .collect::<Vec<_>>()
                        })
                        .collect_vec();

                    if victim_actions.iter().any(|inner| inner.is_empty()) {
                        tracing::trace!("no victim actions found");
                        return None
                    }

                    if victims
                        .iter()
                        .flatten()
                        .map(|v| tree.get_root(*v).unwrap().get_root_action())
                        .filter(|d| !d.is_revert())
                        .any(|d| executor_contract == d.get_to_address())
                    {
                        tracing::trace!("victim address is same as mev executor contract");
                        return None
                    }

                    self.calculate_jit(
                        front_runs,
                        backrun,
                        metadata.clone(),
                        searcher_actions,
                        victim_actions,
                        victim_info,
                    )
                },
            )
            .collect::<Vec<_>>()
    }

    // fn recursive_possible_jits(
    //     &self,
    //     frontrun_info: Vec<TxInfo>,
    //     backrun_info: TxInfo,
    //     metadata: Arc<Metadata>,
    //     searcher_actions: Vec<Vec<Action>>,
    //     // victim
    //     victim_actions: Vec<Vec<Action>>,
    //     victim_info: Vec<Vec<TxInfo>>,
    //     mut recursive: u8,
    // ) -> Option<Vec<Bundle>> {
    //     let mut res = vec![];
    //
    //     if recursive >= 6 {
    //         return None
    //     }
    //     if frontrun_info.len() > 1 {
    //         recursive += 1;
    //         // remove dropped sandwiches
    //         if victim_info.is_empty() || victim_actions.is_empty() {
    //             return None
    //         }
    //
    //         let back_shrink = {
    //             let mut victim_info = victim_info.to_vec();
    //             let mut victim_actions = victim_actions.to_vec();
    //             let mut front_run_info = frontrun_info.to_vec();
    //             victim_info.pop()?;
    //             victim_actions.pop()?;
    //             let back_run_info = frontrun_info.pop()?;
    //
    //             if victim_actions.iter().flatten().count() == 0 {
    //                 return None
    //             }
    //
    //             self.calculate_jit(
    //                 frontrun_info,
    //                 backrun_info,
    //                 metadata.clone(),
    //                 searcher_actions,
    //                 victim_actions,
    //                 victim_info,
    //                 recursive,
    //             )
    //         };
    //
    //         let front_shrink = {
    //             let mut victim_info = victim_info.to_vec();
    //             let mut victim_actions = victim_actions.to_vec();
    //             let mut possible_front_runs_info = frontrun_info.to_vec();
    //             let mut searcher_actions = searcher_actions.to_vec();
    //             // ensure we don't loose the last tx
    //             searcher_actions.push(back_run_actions.to_vec());
    //
    //             victim_info.remove(0);
    //             victim_actions.remove(0);
    //             possible_front_runs_info.remove(0);
    //             searcher_actions.remove(0);
    //
    //             if victim_actions
    //                 .iter()
    //                 .flatten()
    //                 .filter_map(
    //                     |(s, t)| if s.is_empty() && t.is_empty() { None } else {
    // Some(true) },                 )
    //                 .count()
    //                 == 0
    //             {
    //                 return None
    //             }
    //
    //             self.calculate_sandwich(
    //                 tree.clone(),
    //                 metadata.clone(),
    //                 possible_front_runs_info,
    //                 backrun_info,
    //                 searcher_actions,
    //                 victim_info,
    //                 victim_actions,
    //                 recusive,
    //             )
    //         };
    //         if let Some(front) = front_shrink {
    //             res.extend(front);
    //         }
    //         if let Some(back) = back_shrink {
    //             res.extend(back);
    //         }
    //         return Some(res)
    //     }
    //
    //     None
    // }

    //TODO: Clean up JIT inspectors
    fn calculate_jit(
        &self,
        frontrun_info: Vec<TxInfo>,
        backrun_info: TxInfo,
        metadata: Arc<Metadata>,
        searcher_actions: Vec<Vec<Action>>,
        // victim
        victim_actions: Vec<Vec<Action>>,
        victim_info: Vec<Vec<TxInfo>>,
    ) -> Option<Bundle> {
        // grab all mints and burns
        let ((mints, burns), rem): ((Vec<_>, Vec<_>), Vec<_>) = searcher_actions
            .clone()
            .into_iter()
            .flatten()
            .action_split_out((Action::try_mint, Action::try_burn));

        if mints.is_empty() || burns.is_empty() {
            tracing::trace!("missing mints & burns");
            return None
        }

        // assert mints and burns are same pool
        let mut pools = FastHashSet::default();
        mints.iter().for_each(|m| {
            pools.insert(m.pool);
        });
        if !burns.iter().any(|b| pools.contains(&b.pool)) {
            return None
        }

        let mut info_set = frontrun_info.clone();
        info_set.push(backrun_info.clone());

        let mev_addresses: FastHashSet<Address> = collect_address_set_for_accounting(&info_set);

        let deltas = rem
            .into_iter()
            .filter(|f| f.is_transfer() || f.is_eth_transfer())
            .account_for_actions();

        let (rev, has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
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

        let (mut hashes, mut gas_details): (Vec<_>, Vec<_>) = info_set
            .iter()
            .map(|info| info.clone().split_to_storage_info())
            .unzip();

        let (victim_hashes, victim_gas_details): (Vec<_>, Vec<_>) = victim_info
            .into_iter()
            .flatten()
            .map(|info| info.split_to_storage_info())
            .unzip();

        let bribe = self.get_bribes(metadata.clone(), &gas_details);
        let profit = rev
            .map(|rev| rev - &bribe)
            .filter(|_| has_dex_price)
            .unwrap_or_default();

        let mut bundle_hashes = Vec::new();
        bundle_hashes.push(hashes[0]);
        bundle_hashes.extend(victim_hashes.clone());
        bundle_hashes.push(hashes[1]);

        let header = self.utils.build_bundle_header(
            vec![deltas],
            bundle_hashes,
            info_set.last()?,
            profit.to_float(),
            PriceAt::After,
            &gas_details,
            metadata,
            MevType::Jit,
            !has_dex_price,
        );

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

        let jit_details = JitLiquidity {
            frontrun_mint_tx_hash: hashes[0],
            frontrun_mint_gas_details: gas_details[0],
            frontrun_mints: mints,
            victim_swaps_tx_hashes: victim_hashes.clone(),
            victim_swaps,
            victim_swaps_gas_details_tx_hashes: victim_hashes,
            victim_swaps_gas_details: victim_gas_details,
            backrun_burn_tx_hash: hashes.pop()?,
            backrun_burn_gas_details: gas_details.pop()?,
            backrun_burns: burns,
        };

        Some(Bundle { header, data: BundleData::Jit(jit_details) })
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
                    possible_victims.insert(root.tx_hash, vec![]);
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
                    possible_victims.insert(root.tx_hash, vec![]);
                }
            }

            match duplicate_senders.entry(root.head.address) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(root.tx_hash);
                    possible_victims.insert(root.tx_hash, vec![]);
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
                    possible_victims.insert(root.tx_hash, vec![]);
                }
            }

            // Now, for each existing entry in possible_victims, we add the current
            // transaction hash as a potential victim, if it is not the same as
            // the key (which represents another transaction hash)
            for (k, v) in possible_victims.iter_mut() {
                if k != &root.tx_hash {
                    v.push(root.tx_hash);
                }
            }
        }

        let set = Itertools::unique(set.into_values())
            .flat_map(Self::partition_into_gaps)
            .collect::<Vec<_>>();

        // split out
        let tx_set = set
            .iter()
            .filter_map(|jit| {
                // let proper_frontruns = jit.frontrun_txes.iter().any(|tx| {
                //     tree.tx_must_contain_action(*tx, |action| action.is_mint())
                //         .unwrap()
                // });
                // if !(proper_frontruns
                //     && tree
                //         .tx_must_contain_action(jit.backrun_tx, |action| action.is_burn())
                //         .unwrap())
                // {
                //     return None
                // }

                if jit.victims.len() > 20 {
                    return None
                }

                let mut set = vec![jit.backrun_tx];
                set.extend(jit.victims.iter().flatten().cloned());
                set.extend(jit.frontrun_txes.clone());
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
            .filter(|jit| jit.victims.iter().flatten().count() <= 20)
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
            .with_expected_profit_usd(-10.61);

        test_utils.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_multihop_jit() {
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0).await;
        let config = InspectorTxRunConfig::new(Inspectors::Jit)
            .with_dex_prices()
            .needs_tokens(vec![WETH_ADDRESS])
            .with_block(18884329)
            .with_gas_paid_usd(792.89)
            .with_expected_profit_usd(17.9);

        test_utils.run_inspector(config, None).await.unwrap();
    }
}
