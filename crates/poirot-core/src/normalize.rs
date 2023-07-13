use poirot_core::{decode::Parser, trace::TracingClient, action::Action};

use std::{env, error::Error, path::Path, collections::HashMap};

use reth_primitives::{H256, H160};

use phf::phf_map;

static STRUCTURES: phf::Map<&'static str, StructureType> = phf_map! {
    "swap" => StructureType::Swap,
};

#[derive(Clone, Debug)]
pub enum StructureType {
    Swap,
}

// pub enum Structure {
//     Swap(Swap),
// }

pub struct Swap {
    protocol: H160,
}

pub struct Normalizer {
    /// Mapping of a transaction hash to a vector of actions.
    pub actions: HashMap<H256, Vec<Action>>,
}

impl Normalizer {
    pub fn new(actions: Vec<Action>) -> Self {
        let mut actions: HashMap<reth_primitives::H256, Vec<Action>> = HashMap::new();

        for i in actions {
            if let Some(x) = tx_map.get_mut(&i.trace.transaction_hash.unwrap()) {
                (*x).push(i);
            } else {
                tx_map.insert(i.trace.transaction_hash.unwrap(), vec![i]);
            }
        }

        Self { actions }
    }

    pub fn normalize(&self) -> Vec<Structure> {
        for (k, v) in self.actions.iter() {
            self.normalize_actions(v);
        }
    }

    pub fn normalize_actions(&self, actions: Vec<Action>) /*-> Structure*/ {
        println!("{}", STRUCTURES.get(&actions[0].function_name).cloned());
    }
}