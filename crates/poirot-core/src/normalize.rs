/*use crate::structured_trace::StructuredTrace::{self, CALL, CREATE};

use std::collections::HashMap;

use reth_primitives::H256;

use phf::phf_map;

static STRUCTURES: phf::Map<&'static str, StructureType> = phf_map! {
    "swap" => StructureType::Swap,
    "createPool" => StructureType::PoolCreation,
    // Add a bunch more function_name -> StwructureType mappings here - could even auto generate.
};

#[derive(Clone, Debug)]
pub enum StructureType {
    Swap,
    PoolCreation,
}

#[derive(Clone, Debug)]
pub enum Structure {
    Swap(StructuredTrace),
    PoolCreation(StructuredTrace),
}

/// A type of protocol.
/// TODO: Add more, and in addition add detection.
#[derive(Debug, Clone)]
pub enum ProtocolType {
    UniswapV2,
    UniswapV3,
    Curve,
}

pub struct Normalizer {
    /// Mapping of a transaction hash to a vector of actions.
    pub actions: HashMap<H256, Vec<StructuredTrace>>,
}

impl Normalizer {
    pub fn new(actions: Vec<StructuredTrace>) -> Self {
        let mut tx_map: HashMap<reth_primitives::H256, Vec<StructuredTrace>> = HashMap::new();

        for action in actions {
            let trace = match &action {
                StructuredTrace::CALL(call_action) => call_action.trace.clone(),
                StructuredTrace::CREATE(create_action) => create_action.trace.clone(),
            };

            if let Some(x) = tx_map.get_mut(&trace.transaction_hash.unwrap()) {
                (*x).push(action);
            } else {
                tx_map.insert(trace.transaction_hash.unwrap(), vec![action]);
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

    pub fn normalize_actions(&self, actions: Vec<StructuredTrace>) -> Vec<Structure> {
        let mut structures = vec![];

        for action in actions {
            match action {
                StructuredTrace::CALL(call_action) => {
                    if let Some(val) = STRUCTURES.get(&call_action.function_name).cloned() {
                        match val {
                            StructureType::Swap => {
                                structures.push(Structure::Swap(StructuredTrace::CALL(call_action)))
                            }
                            StructureType::PoolCreation => structures
                                .push(Structure::PoolCreation(StructuredTrace::CALL(call_action))),
                        }
                    }
                }
                StructuredTrace::CREATE(create_action) => {
                    // handle CREATE actions if necessary
                    // for now, we are skipping them as per your original code
                }
            }
        }

        structures
    }
}
*/
