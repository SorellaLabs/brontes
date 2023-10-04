use std::sync::Arc;

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

use crate::{Actions, ClassifiedMev, Inspector, Metadata, SpecificMev, TimeTree};

#[derive(Default)]
pub struct JitInspector;

#[async_trait]
impl Inspector for JitInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let iter = tree.roots.iter();

        if iter.len() < 3 {
            return vec![]
        }

        let set = iter
            .into_iter()
            .tuple_windows::<(_, _, _)>()
            .filter_map(|(t1, t2, t3)| {
                if t1.head.address == t3.head.address {
                    Some((
                        t1.head.address,
                        t1.tx_hash,
                        t3.tx_hash,
                        t1.head.data.get_too_address(),
                        t2.tx_hash,
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        set.into_iter()
            .filter_map(|(eoa, tx0, tx1, mev_addr, victim)| {
                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap(),
                ];

                let victim_gas = tree.get_gas_details(victim).unwrap().clone();
                let victim_actions = tree
                    .inspect(victim, |node| node.subactions.iter().any(|action| action.is_swap()));

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .flat_map(|tx| {
                        tree.inspect(tx, |node| {
                            node.subactions.iter().any(|action| {
                                action.is_mint() || action.is_burn() || action.is_collect()
                            })
                        })
                    })
                    .collect::<Vec<Vec<Actions>>>();

                self.calculate_jit(
                    eoa,
                    mev_addr,
                    metadata.clone(),
                    [tx0, tx1],
                    gas,
                    searcher_actions,
                    victim,
                    victim_actions,
                    victim_gas,
                )
            })
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
        victim_tx: H256,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: GasDetails,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let (mints, burns, collect): (
            Vec<Option<NormalizedMint>>,
            Vec<Option<NormalizedBurn>>,
            Vec<Option<NormalizedCollect>>,
        ) = searcher_actions
            .clone()
            .into_iter()
            .flatten()
            .map(|action| match action {
                Actions::Burn(b) => (None, Some(b), None),
                Actions::Mint(m) => (Some(m), None, None),
                Actions::Collect(c) => (None, None, Some(c)),

                _ => unreachable!(),
            })
            .multiunzip();

        let mints = mints.into_iter().flatten().collect::<Vec<_>>();
        let burns = burns.into_iter().flatten().collect::<Vec<_>>();

        let fee_collect = collect.into_iter().flatten().collect::<Vec<_>>();

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

        let pre_profit = jit_fee_pre + burn_pre - mint_pre - &pre_bribe;
        let post_profit = jit_fee_post + burn_post - mint_post - &post_bribe;

        let classified = ClassifiedMev {
            block_number: metadata.block_num,
            tx_hash: txes[0],
            eoa,
            mev_contract: mev_addr,
            mev_profit_collector: mev_addr,
            mev_type: MevType::Jit,
            submission_profit_usd: pre_profit.to_float(),
            finalized_profit_usd: post_profit.to_float(),
            submission_bribe_usd: pre_bribe.to_float(),
            finalized_bribe_usd: post_bribe.to_float(),
        };

        let jit_details = JitLiquidity {
            swaps:            victim_actions.into_iter().flatten().collect::<Vec<_>>(),
            jit_mints:        mints.into_iter().map(Actions::Mint).collect::<Vec<_>>(),
            jit_burns:        burns.into_iter().map(Actions::Burn).collect::<Vec<_>>(),
            mint_tx_hash:     txes[0],
            swap_tx_hash:     victim_tx,
            burn_tx_hash:     txes[1],
            mint_gas_details: searcher_gas_details[0],
            burn_gas_details: searcher_gas_details[1],
            swap_gas_details: victim_gas,
        };

        Some((classified, Box::new(jit_details)))
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
