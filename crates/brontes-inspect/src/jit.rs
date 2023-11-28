use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use brontes_types::{
    classified_mev::{JitLiquidity, MevType},
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint},
    tree::GasDetails,
    ToFloatNearest, ToScaledRational, TOKEN_TO_DECIMALS,
};
use itertools::Itertools;
use malachite::Rational;
use reth_primitives::{Address, H160, H256, U256};
use tracing::{debug, info};

use crate::{Actions, ClassifiedMev, Inspector, Metadata, SpecificMev, TimeTree};

#[derive(Debug)]
struct PossibleJit {
    pub eoa:                   Address,
    pub frontrun_tx:           H256,
    pub backrun_tx:            H256,
    pub mev_executor_contract: Address,
    pub victims:               Vec<H256>,
}
#[derive(Default)]
pub struct JitInspector;

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
                        .flat_map(|tx| {
                            tree.inspect(tx, |node| {
                                node.subactions.iter().any(|action| {
                                    action.is_mint() || action.is_burn() || action.is_collect()
                                })
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
                            Actions::Mint(m) => Some(m.recipient),
                            Actions::Burn(b) => Some(b.from),
                            Actions::Collect(c) => Some(c.from),
                            _ => None,
                        })
                        .collect::<HashSet<_>>();

                    // grab all victim swaps dropping swaps that don't touch addresses with
                    // liquidity deltas
                    let (victims, victim_actions): (Vec<H256>, Vec<Vec<Actions>>) = victims
                        .iter()
                        .map(|victim| {
                            (
                                victim,
                                tree.inspect(*victim, |node| {
                                    node.subactions.iter().any(|action| action.is_swap())
                                })
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            )
                        })
                        .filter(|(_, actions)| {
                            actions
                                .iter()
                                .filter(|s| s.is_swap())
                                .any(|s| liquidity_addresses.contains(&s.force_swap_ref().pool))
                        })
                        .unzip();

                    info!("{:#?}", victims);

                    let victim_gas = victims
                        .iter()
                        .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                        .collect::<Vec<_>>();

                    self.calculate_jit(
                        eoa,
                        mev_executor_contract,
                        metadata.clone(),
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
        txes: [H256; 2],
        searcher_gas_details: [GasDetails; 2],
        searcher_actions: Vec<Vec<Actions>>,
        // victim
        victim_txs: Vec<H256>,
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
        info!("{:#?}", mints);
        info!("{:#?}", burns);
        info!("{:#?}", fee_collect);

        let (jit_fee_pre, jit_fee_post) = self.get_collect_amount(fee_collect, metadata.clone());

        let (mint_pre, mint_post) = self.get_total_pricing(
            mints.iter().map(|mint| (&mint.token, &mint.amount)),
            metadata.clone(),
        );
        let (burn_pre, burn_post) = self.get_total_pricing(
            burns.iter().map(|burn| (&burn.token, &burn.amount)),
            metadata.clone(),
        );

        let (pre_bribe, post_bribe) = self.get_bribes(metadata.clone(), searcher_gas_details);

        let pre_profit = jit_fee_pre - mint_pre - &pre_bribe;
        let post_profit = jit_fee_post - mint_post - &post_bribe;

        info!(?pre_profit, ?post_profit, "pre post jit profit");

        let classified = ClassifiedMev {
            block_number: metadata.block_num,
            tx_hash: txes[0],
            eoa,
            mev_contract: mev_addr,
            mev_profit_collector: vec![mev_addr],
            mev_type: MevType::Jit,
            submission_profit_usd: pre_profit.to_float(),
            finalized_profit_usd: post_profit.to_float(),
            submission_bribe_usd: pre_bribe.to_float(),
            finalized_bribe_usd: post_bribe.to_float(),
        };

        let swaps = victim_actions
            .into_iter()
            .flatten()
            .map(|s| s.force_swap())
            .collect::<Vec<_>>();

        let jit_details = JitLiquidity {
            mint_tx_hash: txes[0],
            mint_gas_details: searcher_gas_details[0],
            jit_mints_index: mints.iter().map(|m| m.index).collect(),
            jit_mints_from: mints.iter().map(|m| m.from).collect(),
            jit_mints_to: mints.iter().map(|m| m.to).collect(),
            jit_mints_recipient: mints.iter().map(|m| m.recipient).collect(),
            jit_mints_token: mints.iter().map(|m| m.token.clone()).collect(),
            jit_mints_amount: mints
                .iter()
                .map(|m| m.amount.clone().into_iter().map(|l| l.to()).collect_vec())
                .collect(),
            swap_tx_hash: victim_txs,
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
            swaps_index: swaps.iter().map(|s| s.index).collect::<Vec<_>>(),
            swaps_from: swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            swaps_pool: swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            swaps_token_in: swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            swaps_token_out: swaps.iter().map(|s| s.token_out).collect::<Vec<_>>(),
            swaps_amount_in: swaps.iter().map(|s| s.amount_in.to()).collect::<Vec<_>>(),
            swaps_amount_out: swaps.iter().map(|s| s.amount_out.to()).collect::<Vec<_>>(),
            burn_tx_hash: txes[1],
            burn_gas_details: searcher_gas_details[1],
            jit_burns_index: burns.iter().map(|m| m.index).collect(),
            jit_burns_from: burns.iter().map(|m| m.from).collect(),
            jit_burns_to: burns.iter().map(|m| m.to).collect(),
            jit_burns_recipient: burns.iter().map(|m| m.recipient).collect(),
            jit_burns_token: burns.iter().map(|m| m.token.clone()).collect(),
            jit_burns_amount: burns
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
        let mut duplicate_senders: HashMap<Address, Vec<H256>> = HashMap::new();
        let mut possible_victims: HashMap<H256, Vec<H256>> = HashMap::new();

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
                        if let Some(victims) = possible_victims.get(&prev_tx_hash) {
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

        info!(?set, "possible jit set");
        set
    }

    fn get_bribes(&self, price: Arc<Metadata>, gas: [GasDetails; 2]) -> (Rational, Rational) {
        let bribe = gas.into_iter().map(|gas| gas.gas_paid()).sum::<u64>();

        price.get_gas_price_usd(bribe)
    }

    fn get_collect_amount(
        &self,
        collect: Vec<NormalizedCollect>,
        metadata: Arc<Metadata>,
    ) -> (Rational, Rational) {
        let (tokens, amount): (Vec<Vec<_>>, Vec<Vec<_>>) =
            collect.into_iter().map(|t| (t.token, t.amount)).unzip();

        let tokens = tokens.into_iter().flatten().collect::<Vec<_>>();
        let amount = amount.into_iter().flatten().collect::<Vec<_>>();

        (
            self.get_liquidity_price(metadata.clone(), &tokens, &amount, |(p, _)| p),
            self.get_liquidity_price(metadata.clone(), &tokens, &amount, |(_, p)| p),
        )
    }

    fn get_total_pricing<'a>(
        &self,
        iter: impl Iterator<Item = (&'a Vec<H160>, &'a Vec<U256>)>,
        metadata: Arc<Metadata>,
    ) -> (Rational, Rational) {
        let (pre, post): (Vec<_>, Vec<_>) = iter
            .map(|(token, amount)| {
                (
                    self.get_liquidity_price(metadata.clone(), token, amount, |(p, _)| p),
                    self.get_liquidity_price(metadata.clone(), token, amount, |(_, p)| p),
                )
            })
            .unzip();
        (pre.into_iter().sum(), post.into_iter().sum())
    }

    fn get_liquidity_price(
        &self,
        metadata: Arc<Metadata>,
        token: &Vec<H160>,
        amount: &Vec<U256>,
        is_pre: impl Fn(&(Rational, Rational)) -> &Rational,
    ) -> Rational {
        assert_eq!(token.len(), amount.len());

        token
            .iter()
            .zip(amount.iter())
            .filter_map(|(token, amount)| {
                Some(
                    is_pre(metadata.token_prices.get(&token)?)
                        * amount.to_scaled_rational(*TOKEN_TO_DECIMALS.get(&token.0)?),
                )
            })
            .sum::<Rational>()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        env,
        str::FromStr,
        time::SystemTime,
    };

    use brontes_classifier::Classifier;
    use brontes_core::test_utils::{init_trace_parser, init_tracing};
    use brontes_database::database::Database;
    use brontes_types::test_utils::write_tree_as_json;
    use malachite::num::{basic::traits::One, conversion::traits::FromSciString};
    use reth_primitives::U256;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;
    use tracing::info;

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
            token_prices:           {
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
            eth_prices:             (
                Rational::try_from_float_simplest(2126.43).unwrap(),
                Rational::try_from_float_simplest(2126.43).unwrap(),
            ),
            mempool_flow:           {
                let mut private = HashSet::new();
                private.insert(
                    H256::from_str(
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
        let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

        let inspector = JitInspector::default();

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("{:#?}", mev);

        println!("jit inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == H256::from_str(
        //
        // "0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de"
        //         )
        //         .unwrap()
        // );
    }
}
