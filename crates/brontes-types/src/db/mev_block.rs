use crate::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(ClassifiedMev, SpecificMev)>,
}
