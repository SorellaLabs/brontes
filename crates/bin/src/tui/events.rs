use brontes_database::Tables;
use brontes_types::mev::{Bundle, MevBlock};
use serde::{Deserialize, Serialize};
use strum::Display;

/// metric event for traces
#[derive(Debug, Clone, Serialize, Display, Deserialize)]
pub enum BrontesData {
    Block((MevBlock, Vec<Bundle>)),
    Init(ProgressUpdate),
}

pub enum ProgressUpdate {
    Global(u16),
    Table((Table, u16)),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
}
