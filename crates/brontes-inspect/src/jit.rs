use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{JitLiquidity, MevType},
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint},
    tree::GasDetails,
    ToFloatNearest, ToScaledRational,
};
use itertools::Itertools;
use malachite::Rational;
use reth_primitives::{Address, B256, U256};

use crate::{
    shared_utils::SharedInspectorUtils, Actions, BlockTree, ClassifiedMev, Inspector, Metadata,
    SpecificMev,
};

#[derive(Debug, PartialEq, Eq, Hash)]
struct PossibleJit {
    pub eoa:                   Address,
    pub frontrun_tx:           B256,
    pub backrun_tx:            B256,
    pub mev_executor_contract: Address,
    pub victims:               Vec<B256>,
}

pub struct JitInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> JitInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait]
impl Inspector for JitInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, SpecificMev)> {
        self.possible_jit_set(tree.clone())
            .into_iter()
            .filter_map(
                |PossibleJit { eoa, frontrun_tx, backrun_tx, mev_executor_contract, victims }| {
                    let searcher_actions = vec![frontrun_tx, backrun_tx]
                        .into_iter()
                        .map(|tx| {
                            tree.collect(tx, |node| {
                                (
                                    node.data.is_mint()
                                        || node.data.is_burn()
                                        || node.data.is_collect(),
                                    node.subactions.iter().any(|action| {
                                        action.is_mint()
                                            || action.is_collect()
                                            || node.data.is_burn()
                                    }),
                                )
                            })
                        })
                        .collect::<Vec<Vec<Actions>>>();

                    if searcher_actions.is_empty() {
                        return None
                    }
                    let gas = [
                        tree.get_gas_details(frontrun_tx).cloned().unwrap(),
                        tree.get_gas_details(backrun_tx).cloned().unwrap(),
                    ];

                    if victims
                        .iter()
                        .map(|v| tree.get_root(*v).unwrap().head.data.clone())
                        .filter(|d| !d.is_revert())
                        .any(|d| mev_executor_contract == d.get_to_address())
                    {
                        return None
                    }

                    // grab all victim swaps dropping swaps that don't touch addresses with
                    let (victims, victim_actions): (Vec<B256>, Vec<Vec<Actions>>) = victims
                        .iter()
                        .map(|victim| {
                            (
                                victim,
                                tree.collect(*victim, |node| {
                                    (
                                        node.data.is_swap(),
                                        node.subactions.iter().any(|action| action.is_swap()),
                                    )
                                }),
                            )
                        })
                        .unzip();

                    if victim_actions.iter().any(|inner| inner.is_empty()) {
                        return None
                    }

                    let victim_gas = victims
                        .iter()
                        .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                        .collect::<Vec<_>>();

                    let idxs = tree.get_root(backrun_tx).unwrap().get_block_position();

                    self.calculate_jit(
                        eoa,
                        mev_executor_contract,
                        metadata.clone(),
                        idxs,
                        [frontrun_tx, backrun_tx],
                        gas,
                        searcher_actions,
                        victims,
                        victim_actions,
                        victim_gas,
                    )
                },
            )
            .collect::<Vec<_>>()
    }
}

impl JitInspector<'_> {
    fn calculate_jit(
        &self,
        eoa: Address,
        mev_addr: Address,
        metadata: Arc<Metadata>,
        back_jit_idx: usize,
        txes: [B256; 2],
        searcher_gas_details: [GasDetails; 2],
        searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txs: Vec<B256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, SpecificMev)> {
        let (mints, burns, collect): (
            Vec<Option<NormalizedMint>>,
            Vec<Option<NormalizedBurn>>,
            Vec<Option<NormalizedCollect>>,
        ) = searcher_actions
            .clone()
            .into_iter()
            .flatten()
            .filter_map(|action| match action {
                Actions::Burn(b) => Some((None, Some(b), None)),
                Actions::Mint(m) => Some((Some(m), None, None)),
                Actions::Collect(c) => Some((None, None, Some(c))),
                _ => None,
            })
            .multiunzip();

        let mints = mints.into_iter().flatten().collect::<Vec<_>>();
        let burns = burns.into_iter().flatten().collect::<Vec<_>>();
        let fee_collect = collect.into_iter().flatten().collect::<Vec<_>>();

        if mints.is_empty() || burns.is_empty() {
            return None
        }

        let jit_fee = self.get_collect_amount(back_jit_idx, fee_collect, metadata.clone());

        let mint = self.get_total_pricing(
            back_jit_idx,
            mints.iter().map(|mint| (&mint.token, &mint.amount)),
            metadata.clone(),
        );

        let bribe = self.get_bribes(metadata.clone(), searcher_gas_details);

        let profit = jit_fee - mint - &bribe;

        let classified = ClassifiedMev {
            mev_tx_index: back_jit_idx as u64,
            block_number: metadata.block_num,
            tx_hash: txes[0],
            eoa,
            mev_contract: mev_addr,
            mev_profit_collector: vec![mev_addr],
            mev_type: MevType::Jit,
            finalized_profit_usd: profit.to_float(),
            finalized_bribe_usd: bribe.to_float(),
        };

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
            frontrun_mint_tx_hash: txes[0],
            frontrun_mint_gas_details: searcher_gas_details[0],
            frontrun_mints: mints,
            victim_swaps_tx_hashes: victim_txs.clone(),
            victim_swaps,
            victim_swaps_gas_details_tx_hashes: victim_txs.clone(),
            victim_swaps_gas_details: victim_gas,
            backrun_burn_tx_hash: txes[1],
            backrun_burn_gas_details: searcher_gas_details[1],
            backrun_burns: burns,
        };

        Some((classified, SpecificMev::Jit(jit_details)))
    }

    fn possible_jit_set(&self, tree: Arc<BlockTree<Actions>>) -> Vec<PossibleJit> {
        let iter = tree.tx_roots.iter();

        if iter.len() < 3 {
            return vec![]
        }

        let mut set: HashSet<PossibleJit> = HashSet::new();
        let mut duplicate_mev_contracts: HashMap<Address, Vec<B256>> = HashMap::new();
        let mut duplicate_senders: HashMap<Address, Vec<B256>> = HashMap::new();

        let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();

        for root in iter {
            if root.head.data.is_revert() {
                continue
            }

            match duplicate_mev_contracts.entry(root.head.data.get_to_address()) {
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
                            if victims.len() >= 1 {
                                // Create
                                set.insert(PossibleJit {
                                    eoa:                   root.head.address,
                                    frontrun_tx:           *prev_tx_hash,
                                    backrun_tx:            root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
                                    victims:               victims.clone(),
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
                            if victims.len() >= 1 {
                                // Create
                                set.insert(PossibleJit {
                                    eoa:                   root.head.address,
                                    frontrun_tx:           *prev_tx_hash,
                                    backrun_tx:            root.tx_hash,
                                    mev_executor_contract: root.head.data.get_to_address(),
                                    victims:               victims.clone(),
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

        set.into_iter().collect()
    }

    fn get_bribes(&self, price: Arc<Metadata>, gas: [GasDetails; 2]) -> Rational {
        let bribe = gas.into_iter().map(|gas| gas.gas_paid()).sum::<u128>();

        price.get_gas_price_usd(bribe)
    }

    fn get_collect_amount(
        &self,
        idx: usize,
        collect: Vec<NormalizedCollect>,
        metadata: Arc<Metadata>,
    ) -> Rational {
        let (tokens, amount): (Vec<Vec<_>>, Vec<Vec<_>>) =
            collect.into_iter().map(|t| (t.token, t.amount)).unzip();

        let tokens = tokens.into_iter().flatten().collect::<Vec<_>>();
        let amount = amount.into_iter().flatten().collect::<Vec<_>>();

        self.get_liquidity_price(idx, metadata.clone(), &tokens, &amount)
    }

    fn get_total_pricing<'a>(
        &self,
        idx: usize,
        iter: impl Iterator<Item = (&'a Vec<Address>, &'a Vec<U256>)>,
        metadata: Arc<Metadata>,
    ) -> Rational {
        iter.map(|(token, amount)| self.get_liquidity_price(idx, metadata.clone(), token, amount))
            .sum()
    }

    fn get_liquidity_price(
        &self,
        idx: usize,
        metadata: Arc<Metadata>,
        token: &Vec<Address>,
        amount: &Vec<U256>,
    ) -> Rational {
        assert_eq!(token.len(), amount.len());

        token
            .iter()
            .zip(amount.iter())
            .filter_map(|(token, amount)| {
                Some(
                    self.inner
                        .get_dex_usd_price(idx, *token, metadata.clone())?
                        * amount.to_scaled_rational(self.inner.try_get_decimals(*token)?),
                )
            })
            .sum::<Rational>()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        str::FromStr,
        time::SystemTime,
    };

    use reth_primitives::U256;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    async fn test_jit() {
        // eth price in usdc
        // 2146.65037178
        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 0.1);
        let config = InspectorTxRunConfig::new(MevType::Jit)
            .with_dex_prices()
            .with_block(18539312)
            .with_expected_gas_used(90.875025)
            .with_expected_profit_usd(-68.34);

        test_utils
            .run_inspector::<JitLiquidity>(config, None)
            .await
            .unwrap();
    }
}
