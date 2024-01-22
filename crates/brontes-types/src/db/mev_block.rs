use crate::classified_mev::{BundleData, BundleHeader, MevBlock};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(BundleHeader, BundleData)>,
}
