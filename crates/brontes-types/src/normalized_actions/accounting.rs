use std::{collections::hash_map::Entry, hash::Hash};

use alloy_primitives::Address;
use malachite::Rational;

use super::{comparison::ActionComparison, Action};
use crate::FastHashMap;

pub type TokenDeltas = FastHashMap<Address, Rational>;
pub type AddressDeltas = FastHashMap<Address, TokenDeltas>;

/// apply's the given actions token deltas to the map;
pub trait TokenAccounting {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas);
}

/// For a given Vector of actions, will go through and apply the token deltas,
/// de-duping them as it is doing the calculations
pub trait ActionAccounting {
    /// for a given list of actions, this will dedup the actions and then apply
    /// the token deltas for it
    fn account_for_actions(self) -> AddressDeltas;
}

fn accounting_calc(accounting: &mut Accounting, next: Action) {
    if accounting
        .accounted_for_actions
        .iter()
        .all(|i| !i.is_same_coverage(&next))
    {
        next.apply_token_deltas(&mut accounting.delta_map);
        accounting.accounted_for_actions.push(next);
    }
}

impl<IT: Iterator<Item = Action>> ActionAccounting for IT {
    /// if the action has already been accounted for then we don't call apply.
    fn account_for_actions(self) -> AddressDeltas {
        let mut accounting = Accounting::new();
        // non transfer swaps
        let mut rem = vec![];

        for next in self {
            if next.is_transfer() {
                rem.push(next);
                continue;
            }
            accounting_calc(&mut accounting, next);
        }

        for next in rem {
            accounting_calc(&mut accounting, next);
        }

        accounting.delta_map
    }
}

/// Holds all accounting info.
pub struct Accounting {
    pub delta_map:             AddressDeltas,
    pub accounted_for_actions: Vec<Action>,
}
impl Default for Accounting {
    fn default() -> Self {
        Self::new()
    }
}

impl Accounting {
    pub fn new() -> Self {
        Self { delta_map: FastHashMap::default(), accounted_for_actions: vec![] }
    }
}

pub fn apply_delta<K: PartialEq + Hash + Eq>(
    address: K,
    token: K,
    amount: Rational,
    delta_map: &mut FastHashMap<K, FastHashMap<K, Rational>>,
) {
    match delta_map.entry(address).or_default().entry(token) {
        Entry::Occupied(mut o) => {
            *o.get_mut() += amount;
        }
        Entry::Vacant(v) => {
            v.insert(amount);
        }
    }
}

#[cfg(test)]
pub mod test {
    // todo: add tests
}
