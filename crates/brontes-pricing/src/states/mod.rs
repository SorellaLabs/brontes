use brontes_types::FastHashMap;
use frayed_ends::FrayedEnds;

pub mod frayed_ends;
pub mod loading;
pub mod recreate;
pub mod verifying;

pub struct StateTracker {
    current_block_states: FastHashMap<u64, Vec<States>>,
}

pub enum States {
    FrayedEnds(FrayedEnds),
    Loading(),
    Recreating(),
    Verifying(),
}
