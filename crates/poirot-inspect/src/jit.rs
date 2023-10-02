use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use async_trait::async_trait;
use itertools::Itertools;
use malachite::Rational;
use poirot_types::{
    classified_mev::JitLiquidity,
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedTransfer},
    tree::GasDetails,
    ToScaledRational, TOKEN_TO_DECIMALS,
};
use reth_primitives::{Address, H160, H256, U256};

use crate::{Actions, ClassifiedMev, Inspector, Metadata, SpecificMev, TimeTree};

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

        // could tech be more than one victim but unlikely
        let mut set = Vec::new();
        let mut pairs = HashMap::new();
        let mut possible_victims: HashMap<H256, Vec<H256>> = HashMap::new();

        for root in iter {
            match pairs.entry(root.head.address) {
                Entry::Vacant(v) => {
                    v.insert(root.tx_hash);
                    possible_victims.insert(root.tx_hash, vec![]);
                }
                Entry::Occupied(o) => {
                    let entry: H256 = o.remove();
                    if let Some(victims) = possible_victims.remove(&entry) {
                        set.push((
                            root.head.address,
                            entry,
                            root.tx_hash,
                            root.head.data.get_too_address(),
                            victims,
                        ));
                    }
                }
            }

            possible_victims.iter_mut().for_each(|(_, v)| {
                v.push(root.tx_hash);
            });
        }

        set.into_iter()
            .filter_map(|(eoa, tx0, tx1, mev_addr, victim)| {
                let gas = [
                    tree.get_gas_details(tx0).cloned().unwrap(),
                    tree.get_gas_details(tx1).cloned().unwrap(),
                ];

                let victim_gas = victim
                    .iter()
                    .map(|victim| tree.get_gas_details(*victim).cloned().unwrap())
                    .collect::<Vec<_>>();

                let victim_actions = victim
                    .iter()
                    .map(|victim| {
                        tree.inspect(*victim, |node| {
                            node.subactions.iter().any(|action| action.is_swap())
                        })
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                    })
                    .collect::<Vec<Vec<Actions>>>();

                let searcher_actions = vec![tx0, tx1]
                    .into_iter()
                    .flat_map(|tx| {
                        tree.inspect(tx, |node| {
                            node.subactions.iter().any(|action| {
                                action.is_mint() || action.is_burn() || action.is_transfer()
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
        victim_txes: Vec<H256>,
        victim_actions: Vec<Vec<Actions>>,
        victim_gas: Vec<GasDetails>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let (mints, burns, transfers): (
            Vec<Option<NormalizedMint>>,
            Vec<Option<NormalizedBurn>>,
            Vec<Option<NormalizedTransfer>>,
        ) = searcher_actions
            .into_iter()
            .flatten()
            .map(|action| match action {
                Actions::Burn(b) => (None, Some(b), None),
                Actions::Mint(m) => (Some(m), None, None),
                Actions::Transfer(t) => (None, None, Some(t)),

                _ => unreachable!(),
            })
            .multiunzip();

        let mints = mints.into_iter().flatten().collect::<Vec<_>>();
        let burns = burns.into_iter().flatten().collect::<Vec<_>>();

        let fee_collection_transfers = transfers
            .into_iter()
            .flatten()
            .filter(|transfer| {
                mints
                    .iter()
                    .find(|m| m.token.contains(&transfer.token) && m.to == transfer.from)
                    .is_some()
                    || burns
                        .iter()
                        .find(|b| b.token.contains(&transfer.token) && b.from == transfer.from)
                        .is_some()
            })
            .collect::<Vec<_>>();

        let (jit_fee_pre, jit_fee_post) =
            self.get_transfer_price(fee_collection_transfers, metadata.clone());

        let (mint_pre, mint_post) = self.get_total_pricing(
            mints.iter().map(|mint| (&mint.token, &mint.amount)),
            metadata.clone(),
        );
        let (burn_pre, burn_post) = self.get_total_pricing(
            burns.iter().map(|burn| (&burn.token, &burn.amount)),
            metadata.clone(),
        );

        None
    }

    fn get_transfer_price(
        &self,
        transfers: Vec<NormalizedTransfer>,
        metadata: Arc<Metadata>,
    ) -> (Rational, Rational) {
        let (tokens, amount) = transfers.into_iter().map(|t| (t.token, t.amount)).unzip();

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
