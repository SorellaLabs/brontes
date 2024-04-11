use brontes_types::mev::{Bundle, MevBlock};
use strum::Display;

use crate::Tables;

#[derive(Debug, Clone, Display)]
pub enum TuiUpdate {
    Block((MevBlock, Vec<Bundle>)),
    Init(ProgressUpdate),
}

#[derive(Debug, Clone, Display)]
pub enum ProgressUpdate {
    Global(ProgressBar),
    Table((Tables, ProgressBar)),
}

#[derive(Debug, Clone)]
pub struct ProgressBar {
    pub position: usize,
    pub target:   usize,
}
