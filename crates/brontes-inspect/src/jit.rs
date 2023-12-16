use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use brontes_types::{
    classified_mev::{JitLiquidity, MevType},
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint},
    tree::GasDetails,
    try_get_decimals, ToFloatNearest, ToScaledRational,
};
use itertools::Itertools;
use malachite::Rational;
use reth_primitives::{Address, B256, U256};

use crate::{
    shared_utils::SharedInspectorUtils, Actions, ClassifiedMev, Inspector, Metadata, SpecificMev,
    TimeTree,
};

#[derive(Debug)]
struct PossibleJit {
    pub eoa:                   Address,
    pub frontrun_tx:           B256,
    pub backrun_tx:            B256,
    pub mev_executor_contract: Address,
    pub victims:               Vec<B256>,
}

pub struct JitInspector {
    inner: SharedInspectorUtils,
}

impl JitInspector {
    pub fn new(quote: Address) -> Self {
        Self { inner: SharedInspectorUtils::new(quote) }
    }
}

#[async_trait]
impl Inspector for JitInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
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

                    // grab all the pools that had liquidity events on them
                    let liquidity_addresses = searcher_actions
                        .iter()
                        .flatten()
                        .filter_map(|action| match action {
                            Actions::Mint(m) => Some(m.to),
                            Actions::Burn(b) => Some(b.to),
                            Actions::Collect(c) => Some(c.to),
                            _ => None,
                        })
                        .collect::<HashSet<_>>();

                    // grab all victim swaps dropping swaps that don't touch addresses with
                    // liquidity deltas
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
                        .filter(|(_, actions)| {
                            actions
                                .iter()
                                .any(|s| liquidity_addresses.contains(&s.force_swap_ref().pool))
                        })
                        .unzip();

                    let victim_gas = victims
                        .iter()
                        .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                        .collect::<Vec<_>>();

                    let idxs = [
                        tree.get_root(frontrun_tx).unwrap().get_block_position(),
                        tree.get_root(backrun_tx).unwrap().get_block_position(),
                    ];

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

impl JitInspector {
    fn calculate_jit(
        &self,
        eoa: Address,
        mev_addr: Address,
        metadata: Arc<Metadata>,
        idxs: [usize; 2],
        txes: [B256; 2],
        searcher_gas_details: [GasDetails; 2],
        searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txs: Vec<B256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
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

        let jit_fee = self.get_collect_amount(idxs[1], fee_collect, metadata.clone());

        let mint = self.get_total_pricing(
            idxs[0],
            mints.iter().map(|mint| (&mint.token, &mint.amount)),
            metadata.clone(),
        );

        let bribe = self.get_bribes(metadata.clone(), searcher_gas_details);

        let profit = jit_fee - mint - &bribe;

        let classified = ClassifiedMev {
            block_number: metadata.block_num,
            tx_hash: txes[0],
            eoa,
            mev_contract: mev_addr,
            mev_profit_collector: vec![mev_addr],
            mev_type: MevType::Jit,
            finalized_profit_usd: profit.to_float(),
            finalized_bribe_usd: bribe.to_float(),
        };

        let swaps = victim_actions
            .into_iter()
            .flatten()
            .filter(|s| s.is_swap())
            .map(|s| s.force_swap())
            .collect::<Vec<_>>();

        let jit_details = JitLiquidity {
            mint_tx_hash: txes[0],
            mint_gas_details: searcher_gas_details[0],
            jit_mints_index: mints.iter().map(|m| m.index as u16).collect(),
            jit_mints_from: mints.iter().map(|m| m.from).collect(),
            jit_mints_to: mints.iter().map(|m| m.to).collect(),
            jit_mints_recipient: mints.iter().map(|m| m.recipient).collect(),
            jit_mints_tokens: mints.iter().map(|m| m.token.clone()).collect(),
            jit_mints_amounts: mints
                .iter()
                .map(|m| m.amount.clone().into_iter().map(|l| l.to()).collect_vec())
                .collect(),
            victim_swap_tx_hashes: victim_txs.clone(),
            victim_swaps_tx_hash: victim_txs,
            victim_gas_details_gas_used: victim_gas.iter().map(|s| s.gas_used).collect_vec(),
            victim_gas_details_priority_fee: victim_gas
                .iter()
                .map(|s| s.priority_fee)
                .collect_vec(),
            victim_gas_details_coinbase_transfer: victim_gas
                .iter()
                .map(|s| s.coinbase_transfer)
                .collect_vec(),
            victim_gas_details_effective_gas_price: victim_gas
                .iter()
                .map(|s| s.effective_gas_price)
                .collect_vec(),
            victim_swaps_index: swaps.iter().map(|s| s.index as u16).collect::<Vec<_>>(),
            victim_swaps_from: swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            victim_swaps_pool: swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            victim_swaps_token_in: swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            victim_swaps_token_out: swaps.iter().map(|s| s.token_out).collect::<Vec<_>>(),
            victim_swaps_amount_in: swaps.iter().map(|s| s.amount_in.to()).collect::<Vec<_>>(),
            victim_swaps_amount_out: swaps.iter().map(|s| s.amount_out.to()).collect::<Vec<_>>(),
            burn_tx_hash: txes[1],
            burn_gas_details: searcher_gas_details[1],
            jit_burns_index: burns.iter().map(|m| m.index as u16).collect(),
            jit_burns_from: burns.iter().map(|m| m.from).collect(),
            jit_burns_to: burns.iter().map(|m| m.to).collect(),
            jit_burns_recipient: burns.iter().map(|m| m.recipient).collect(),
            jit_burns_tokens: burns.iter().map(|m| m.token.clone()).collect(),
            jit_burns_amounts: burns
                .iter()
                .map(|m| m.amount.clone().into_iter().map(|l| l.to()).collect_vec())
                .collect(),
        };

        Some((classified, Box::new(jit_details)))
    }

    fn possible_jit_set(&self, tree: Arc<TimeTree<Actions>>) -> Vec<PossibleJit> {
        let iter = tree.roots.iter();

        if iter.len() < 3 {
            return vec![]
        }

        let mut set: Vec<PossibleJit> = Vec::new();
        let mut duplicate_senders: HashMap<Address, Vec<B256>> = HashMap::new();
        let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::new();

        for root in iter {
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
                            if victims.len() >= 2 {
                                // Create
                                set.push(PossibleJit {
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

        set
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
                        .get_usd_price_dex_avg(idx, *token, metadata.clone())?
                        * amount.to_scaled_rational(try_get_decimals(&token.0 .0)?),
                )
            })
            .sum::<Rational>()
    }
}
/*
#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        str::FromStr,
        time::SystemTime,
    };

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::{init_trace_parser, init_tracing};
    use brontes_database::database::Database;
    use malachite::num::{basic::traits::One, conversion::traits::FromSciString};
    use reth_primitives::U256;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    fn get_metadata() -> Metadata {
        // 2126.43
        Metadata {
            block_num:              18539312,
            block_hash:             U256::from_str_radix(
                "57968198764731c3fcdb0caff812559ce5035aabade9e6bcb2d7fcee29616729",
                16,
            )
            .unwrap(),
            relay_timestamp:        1696271963129, // Oct 02 2023 18:39:23 UTC
            p2p_timestamp:          1696271964134, // Oct 02 2023 18:39:24 UTC
            proposer_fee_recipient: Address::from_str("0x388c818ca8b9251b393131c08a736a67ccb19297")
                .unwrap(),
            proposer_mev_reward:    11769128921907366414,
            cex_quotes:             {
                let mut prices = HashMap::new();

                prices.insert(
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                    (
                        Rational::try_from_float_simplest(2126.43).unwrap(),
                        Rational::try_from_float_simplest(2126.43).unwrap(),
                    ),
                );

                // SMT
                prices.insert(
                    Address::from_str("0xb17548c7b510427baac4e267bea62e800b247173").unwrap(),
                    (
                        Rational::try_from_float_simplest(0.09081931).unwrap(),
                        Rational::try_from_float_simplest(0.09081931).unwrap(),
                    ),
                );

                // APX
                prices.insert(
                    Address::from_str("0xed4e879087ebd0e8a77d66870012b5e0dffd0fa4").unwrap(),
                    (
                        Rational::try_from_float_simplest(0.00004047064).unwrap(),
                        Rational::try_from_float_simplest(0.00004047064).unwrap(),
                    ),
                );
                // FTT
                prices.insert(
                    Address::from_str("0x50d1c9771902476076ecfc8b2a83ad6b9355a4c9").unwrap(),
                    (
                        Rational::try_from_float_simplest(1.9358).unwrap(),
                        Rational::try_from_float_simplest(1.9358).unwrap(),
                    ),
                );

                prices
            },
            eth_prices:             (Rational::try_from_float_simplest(2126.43).unwrap(),),
            mempool_flow:           {
                let mut private = HashSet::new();
                private.insert(
                    B256::from_str(
                        "0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295",
                    )
                    .unwrap(),
                );
                private
            },
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_jit() {
        init_tracing();
        dotenv::dotenv().ok();
        // testing https://eigenphi.io/mev/ethereum/tx/0x96a1decbb3787fbe26de84e86d6c2392f7ab7b31fb33f685334d49db2624a424
        // This is a jit sandwich, however we are just trying to detect the jit portion
        let block_num = 18539312;

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let classifier = Classifier::new();

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = get_metadata();

        let tx = block.0.clone().into_iter().take(20).collect::<Vec<_>>();
        let (missing_token_decimals, tree) = classifier.build_tree(tx, block.1);

        let tree = Arc::new(tree);

        let USDC = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        let inspector = Box::new(JitInspector::new(USDC)) as Box<dyn Inspector>;

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("{:#?}", mev);

        println!("jit inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == B256::from_str(
    }
}
*/
