pub mod atomic_backrun;
pub mod cex_dex;
pub mod daddy_inspector;
pub mod sandwich;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use malachite::Rational;
use poirot_database::Metadata;
use poirot_types::{
    classified_mev::{ClassifiedMev, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree,
    ToScaledRational, TOKEN_TO_DECIMALS,
};
use reth_primitives::Address;
use tracing::error;

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)>;

    /// Calculates the swap deltas. if transfers are also passed in. we also
    /// move those deltas on the map around accordingly.
    /// NOTE: the upper level inspector needs to know if the transfer is related
    /// to the underlying swap. action otherwise you could get misreads
    fn calculate_swap_deltas(
        &self,
        actions: &Vec<Vec<Actions>>,
    ) -> HashMap<Address, HashMap<Address, Rational>> {
        let mut transfers = Vec::new();

        // address and there token delta's
        let mut deltas = HashMap::new();
        for action in actions.into_iter().flatten() {
            if let Actions::Swap(swap) = action {
                let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
                    error!(missing_token=?swap.token_in, "missing token in token to decimal map");
                    continue
                };

                let Some(decimals_out) = TOKEN_TO_DECIMALS.get(&swap.token_out.0) else {
                    error!(missing_token=?swap.token_in, "missing token in token to decimal map");
                    continue
                };

                let adjusted_in = -swap.amount_in.to_scaled_rational(*decimals_in);
                let adjusted_out = swap.amount_out.to_scaled_rational(*decimals_out);

                match deltas.entry(swap.from) {
                    Entry::Occupied(mut o) => {
                        let inner: &mut HashMap<Address, Rational> = o.get_mut();

                        apply_entry(swap.token_in, adjusted_in, inner);
                        apply_entry(swap.token_out, adjusted_out, inner);
                    }
                    Entry::Vacant(v) => {
                        let mut default = HashMap::default();
                        default.insert(swap.token_in, adjusted_in);
                        default.insert(swap.token_out, adjusted_out);

                        v.insert(default);
                    }
                }
            } else if let Actions::Transfer(transfer) = action {
                transfers.push(transfer);
            }
        }

        loop {
            let mut changed = false;

            transfers = transfers
                .into_iter()
                .filter_map(|transfer| {
                    let Some(decimals) = TOKEN_TO_DECIMALS.get(&transfer.token.0) else {
                        error!(missing_token=?transfer.token, "missing token in token to decimal map");
                        return None;
                    };
                    let adjusted_amount = transfer.amount.to_scaled_rational(*decimals);

                    if let Some(from_token_map) = deltas.get_mut(&transfer.from) {
                        changed = true;
                        apply_entry(transfer.token, -adjusted_amount.clone(), from_token_map);
                    } else {
                        return Some(transfer)
                    }

                    let to_token_map = deltas.entry(transfer.to).or_default();
                    apply_entry(transfer.token, adjusted_amount, to_token_map);

                    return None
                })
                .collect::<Vec<_>>();

            if changed == false {
                break
            }
        }
        deltas
    }
    /// Given the deltas, metadata, and a time selector, returns the address
    /// with the highest positive usd delta calculated using CEX prices. This is
    /// useful in scenarios where we want to find the end address that
    /// collects the returns of the underlying mev, which isn't always the
    /// address / contract that executed the mev.S
    fn get_best_usd_delta(
        &self,
        deltas: HashMap<Address, HashMap<Address, Rational>>,
        metadata: Arc<Metadata>,
        time_selector: Box<dyn Fn(&(Rational, Rational)) -> &Rational>,
    ) -> Option<(Address, Rational)> {
        deltas
            .clone()
            .into_iter()
            .map(|(caller, tokens)| {
                let summed_value = tokens
                    .into_iter()
                    .map(|(address, mut value)| {
                        if let Some(price) = metadata.token_prices.get(&address) {
                            value *= time_selector(price);
                        }
                        value
                    })
                    .sum::<Rational>();
                (caller, summed_value)
            })
            .max_by(|x, y| x.1.cmp(&y.1))
    }
    //TODO: I was realising, we don't acc need this if we can have the db structs
    // be generic over actions TODO: becauswe we are already querying the
    // interesting state from the tree so we already know what the actions are and
    // can easily classify them as such fn get_relevant_action<F,
    // Action>(actions: Vec<Actions>, call: F) -> Option<Action> where
    //   F: Fn(&Node<V>) -> bool + Send + Sync,
}

fn apply_entry(token: Address, amount: Rational, token_map: &mut HashMap<Address, Rational>) {
    match token_map.entry(token) {
        Entry::Occupied(mut o) => {
            *o.get_mut() += amount;
        }
        Entry::Vacant(v) => {
            v.insert(amount);
        }
    }
}
