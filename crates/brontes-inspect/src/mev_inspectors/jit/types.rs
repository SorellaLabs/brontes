use alloy_primitives::{Address, B256};
use brontes_types::{FastHashMap, TxInfo};

#[derive(Debug)]
pub struct PossibleJitWithInfo {
    pub front_runs:  Vec<TxInfo>,
    pub backrun:     TxInfo,
    pub victim_info: Vec<Vec<TxInfo>>,
    pub inner:       PossibleJit,
}
impl PossibleJitWithInfo {
    pub fn from_jit(ps: PossibleJit, info_set: &FastHashMap<B256, TxInfo>) -> Option<Self> {
        let backrun = info_set.get(&ps.backrun_tx).cloned()?;
        let mut frontruns = vec![];

        for fr in &ps.frontrun_txes {
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

        Some(PossibleJitWithInfo {
            front_runs: frontruns,
            backrun,
            victim_info: victims,
            inner: ps,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct PossibleJit {
    pub eoa:               Address,
    pub frontrun_txes:     Vec<B256>,
    pub backrun_tx:        B256,
    pub executor_contract: Address,
    pub victims:           Vec<Vec<B256>>,
}
