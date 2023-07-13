use crate::{decode::Parser, trace::TracingClient, action::Action};

use std::{env, error::Error, path::Path, collections::HashMap};

use reth_primitives::{H256, H160};

use phf::phf_map;

static STRUCTURES: phf::Map<&'static str, StructureType> = phf_map! {
    "swap" => StructureType::Swap,
    // Add a bunch more function_name -> StructureType mappings here - could even auto generate.
};

#[derive(Clone, Debug)]
pub enum StructureType {
    Swap,
}

pub enum Structure {
    Swap(Action),
}

pub struct Normalizer {
    /// Mapping of a transaction hash to a vector of actions.
    pub actions: HashMap<H256, Vec<Action>>,
}

impl Normalizer {
    pub fn new(actions: Vec<Action>) -> Self {
        let mut tx_map: HashMap<reth_primitives::H256, Vec<Action>> = HashMap::new();

        for i in actions {
            if let Some(x) = tx_map.get_mut(&i.trace.transaction_hash.unwrap()) {
                (*x).push(i);
            } else {
                tx_map.insert(i.trace.transaction_hash.unwrap(), vec![i]);
            }
        }

        Self { actions: tx_map }
    }

    pub fn normalize(&self) -> Vec<Vec<Structure>> {
        let mut normalized = vec![];

        for (_, v) in self.actions.iter() {
            normalized.push(self.normalize_actions(v.clone()));
        }

        normalized
    }

    pub fn normalize_actions(&self, actions: Vec<Action>) -> Vec<Structure> {
        let mut structures = vec![];

        for i in actions {
            match STRUCTURES.get(&i.function_name).cloned() {
                Some(val) => {
                    match val {
                        StructureType::Swap => structures.push(Structure::Swap(i)),
                    }
                },
                None => (),
            }
        }

        structures
    }
}