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
#[allow(unused)]
use clickhouse::{fixed_string::FixedString, row::*};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};

use crate::{
    shared_utils::SharedInspectorUtils, Action, BlockTree, BundleData, Inspector, Metadata,
};

struct PossibleJitWithInfo {
    pub searcher_info: [TxInfo; 2],
    pub victim_info:   Vec<TxInfo>,
    pub inner:         PossibleJit,
}
impl PossibleJitWithInfo {
    pub fn from_jit(ps: PossibleJit, info_set: &FastHashMap<B256, TxInfo>) -> Option<Self> {
        let searcher =
            [info_set.get(&ps.frontrun_tx).cloned()?, info_set.get(&ps.backrun_tx).cloned()?];

        let mut victims = Vec::with_capacity(ps.victims.len());
        for victim in &ps.victims {
            victims.push(info_set.get(victim).cloned()?);
        }

        Some(PossibleJitWithInfo {
            searcher_info: searcher,
            victim_info:   victims,
            inner:         ps,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct PossibleJit {
    pub eoa:               Address,
    pub frontrun_tx:       B256,
    pub backrun_tx:        B256,
    pub executor_contract: Address,
    pub victims:           Vec<B256>,
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

    fn process_tree(&self, tree: Arc<BlockTree<Action>>, metadata: Arc<Metadata>) -> Self::Result {
        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::Jit, || {
                    self.process_tree_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.process_tree_inner(tree, metadata))
    }
}

impl<DB: LibmdbxReader> JitInspector<'_, DB> {
    fn process_tree_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        self.possible_jit_set(tree.clone())
            .into_iter()
            .filter_map(
                |PossibleJitWithInfo {
                     inner: PossibleJit { frontrun_tx, backrun_tx, executor_contract, victims, .. },
                     victim_info,
                     searcher_info,
                 }| {
                    let searcher_actions = [frontrun_tx, backrun_tx]
                        .iter()
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

                    tracing::trace!(?frontrun_tx, ?backrun_tx, "checking if jit");

                    if searcher_actions.is_empty() {
                        tracing::trace!("no searcher actions found");
                        return None
                    }

                    let victim_actions = victims
                        .iter()
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
                        .map(|v| tree.get_root(*v).unwrap().get_root_action())
                        .filter(|d| !d.is_revert())
                        .any(|d| executor_contract == d.get_to_address())
                    {
                        tracing::trace!("victim address is same as mev executor contract");
                        return None
                    }

                    self.calculate_jit(
                        searcher_info,
                        metadata.clone(),
                        searcher_actions,
                        victim_actions,
                        victim_info,
                    )
                },
            )
            .collect::<Vec<_>>()
    }

    //TODO: Clean up JIT inspectors
    fn calculate_jit(
        &self,
        info: [TxInfo; 2],
        metadata: Arc<Metadata>,
        searcher_actions: Vec<Vec<Action>>,
        // victim
        victim_actions: Vec<Vec<Action>>,
        victim_info: Vec<TxInfo>,
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

        let mev_addresses: FastHashSet<Address> = collect_address_set_for_accounting(&info);

        let deltas = rem
            .into_iter()
            .filter(|f| f.is_transfer() || f.is_eth_transfer())
            .account_for_actions();

        let (rev, has_dex_price) = if let Some(rev) = self.utils.get_deltas_usd(
            info[1].tx_index,
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

        let (hashes, gas_details): (Vec<_>, Vec<_>) = info
            .iter()
            .map(|info| info.clone().split_to_storage_info())
            .unzip();

        let (victim_hashes, victim_gas_details): (Vec<_>, Vec<_>) = victim_info
            .into_iter()
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
            &info[1],
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
            backrun_burn_tx_hash: hashes[1],
            backrun_burn_gas_details: gas_details[1],
            backrun_burns: burns,
        };

        Some(Bundle { header, data: BundleData::Jit(jit_details) })
    }

    fn possible_jit_set(&self, tree: Arc<BlockTree<Action>>) -> Vec<PossibleJitWithInfo> {
        let iter = tree.tx_roots.iter();

        if iter.len() < 3 {
            return vec![]
        }

        let mut set: FastHashSet<PossibleJit> = FastHashSet::default();
        let mut duplicate_mev_contracts: FastHashMap<Address, Vec<B256>> = FastHashMap::default();
        let mut duplicate_senders: FastHashMap<Address, Vec<B256>> = FastHashMap::default();

        let mut possible_victims: FastHashMap<B256, Vec<B256>> = FastHashMap::default();

        for root in iter {
            if root.get_root_action().is_revert() {
                continue
            }

            match duplicate_mev_contracts.entry(root.get_to_address()) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(vec![root.tx_hash]);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(mut o) => {
                    let prev_tx_hashes = o.get();

                    for prev_tx_hash in prev_tx_hashes {
                        // Find the victims between the previous and the current transaction
                        if let Some(victims) = possible_victims.get(prev_tx_hash) {
                            if !victims.is_empty() {
                                // Create
                                set.insert(PossibleJit {
                                    eoa:               root.head.address,
                                    frontrun_tx:       *prev_tx_hash,
                                    backrun_tx:        root.tx_hash,
                                    executor_contract: root.get_to_address(),
                                    victims:           victims.clone(),
                                });
                            }
                        }
                    }
                    // Add current transaction hash to the list of transactions for this sender
                    o.get_mut().push(root.tx_hash);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
            }

            match duplicate_senders.entry(root.head.address) {
                // If we have not seen this sender before, we insert the tx hash into the map
                Entry::Vacant(v) => {
                    v.insert(vec![root.tx_hash]);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(mut o) => {
                    let prev_tx_hashes = o.get();

                    for prev_tx_hash in prev_tx_hashes {
                        // Find the victims between the previous and the current transaction
                        if let Some(victims) = possible_victims.get(prev_tx_hash) {
                            if !victims.is_empty() {
                                // Create
                                set.insert(PossibleJit {
                                    eoa:               root.head.address,
                                    frontrun_tx:       *prev_tx_hash,
                                    backrun_tx:        root.tx_hash,
                                    executor_contract: root.get_to_address(),
                                    victims:           victims.clone(),
                                });
                            }
                        }
                    }
                    // Add current transaction hash to the list of transactions for this sender
                    o.get_mut().push(root.tx_hash);
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
        // split out
        let tx_set = set
            .iter()
            .filter_map(|jit| {
                if !(tree
                    .tx_must_contain_action(jit.frontrun_tx, |action| action.is_mint())
                    .unwrap()
                    && tree
                        .tx_must_contain_action(jit.backrun_tx, |action| action.is_burn())
                        .unwrap())
                {
                    return None
                }

                if jit.victims.len() > 20 {
                    return None
                }

                let mut set = vec![jit.frontrun_tx, jit.backrun_tx];
                set.extend(jit.victims.clone());
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
            .filter(|jit| jit.victims.len() <= 20)
            .filter_map(|jit| PossibleJitWithInfo::from_jit(jit, &tx_info_map))
            .collect_vec()
    }

    fn get_bribes(&self, price: Arc<Metadata>, gas: &[GasDetails]) -> Rational {
        let bribe = gas.iter().map(|gas| gas.gas_paid()).sum::<u128>();

        price.get_gas_price_usd(bribe, self.utils.quote)
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
            .with_expected_profit_usd(-71.92);

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
            .with_expected_profit_usd(-10.61);

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
}
