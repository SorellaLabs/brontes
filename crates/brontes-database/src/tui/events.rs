use brontes_types::mev::{Bundle, MevBlock};
use strum::Display;

use crate::Tables;

#[derive(Debug, Clone, Display)]
pub enum BrontesData {
    Block((MevBlock, Vec<Bundle>)),
    Init(ProgressUpdate),
}

#[derive(Debug, Clone, Display)]
pub enum ProgressUpdate {
    Global(u16),
    Table((Tables, u16)),
}

#[derive(Debug, Clone, PartialEq, Eq, Display)]
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
