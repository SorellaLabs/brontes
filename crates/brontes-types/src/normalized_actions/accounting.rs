use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};

use alloy_primitives::Address;
use malachite::Rational;

use super::{comparison::ActionComparison, Actions};

pub type TokenDeltas = HashMap<Address, Rational>;
pub type AddressDeltas = HashMap<Address, TokenDeltas>;

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

impl<IT: Iterator<Item = Actions>> ActionAccounting for IT {
    /// if the action has already been accounted for then we don't call apply.
    fn account_for_actions(self) -> AddressDeltas {
        let mut accounting = Accounting::new();
        // gotta do it this way due to borrow checker
        for next in self {
            if accounting
                .accounted_for_actions
                .iter()
                .all(|i| !i.is_same_coverage(&next))
            {
                next.apply_token_deltas(&mut accounting.delta_map);
                accounting.accounted_for_actions.push(next);
            }
        }

        accounting.delta_map
    }
}

/// Holds all accounting info.
pub struct Accounting {
    pub delta_map:             AddressDeltas,
    pub accounted_for_actions: Vec<Actions>,
}
impl Default for Accounting {
    fn default() -> Self {
        Self::new()
    }
}

impl Accounting {
    pub fn new() -> Self {
        Self { delta_map: HashMap::new(), accounted_for_actions: vec![] }
    }
}

pub fn apply_delta<K: PartialEq + Hash + Eq>(
    address: K,
    token: K,
    amount: Rational,
    delta_map: &mut HashMap<K, HashMap<K, Rational>>,
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
