use crate::classified_mev::{Bundle, MevBlock};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<Bundle>,
}
