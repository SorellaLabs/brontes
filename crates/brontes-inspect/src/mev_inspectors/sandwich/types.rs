use std::hash::Hash;

use alloy_primitives::{Address, B256};
use brontes_types::{FastHashMap, TxInfo};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PossibleSandwich {
    pub eoa:                   Address,
    pub possible_frontruns:    Vec<B256>,
    pub possible_backrun:      B256,
    pub mev_executor_contract: Address,
    // Mapping of possible frontruns to the set of possible victims.
    // By definition the victims of latter transactions can also be victims of the former
    pub victims:               Vec<Vec<B256>>,
}

pub struct PossibleSandwichWithTxInfo {
    pub inner:                   PossibleSandwich,
    pub possible_frontruns_info: Vec<TxInfo>,
    pub possible_backrun_info:   TxInfo,
    pub victims_info:            Vec<Vec<TxInfo>>,
}

impl PossibleSandwichWithTxInfo {
    pub fn from_ps(ps: PossibleSandwich, info_set: &FastHashMap<B256, TxInfo>) -> Option<Self> {
        let backrun = info_set.get(&ps.possible_backrun).cloned()?;
        let mut frontruns = vec![];

        for fr in &ps.possible_frontruns {
            frontruns.push(info_set.get(fr).cloned()?);
        }

        let mut victims = vec![];
        for victim in &ps.victims {
            let mut set = vec![];
            for v in victim {
                set.push(info_set.get(v).cloned()?);
            }
            victims.push(set);
        }

        Some(PossibleSandwichWithTxInfo {
            possible_backrun_info:   backrun,
            possible_frontruns_info: frontruns,
            victims_info:            victims,
            inner:                   ps,
        })
    }
}
