use brontes_types::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};
use sorella_db_databases::clickhouse::{self, Row};

use super::LibmdbxData;
use crate::{tables::MevBlocks, CompressedTable};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct MevBlocksData {
    pub block_number: u64,
    pub mev_blocks:   MevBlockWithClassified,
}

impl LibmdbxData<MevBlocks> for MevBlocksData {
    fn into_key_val(
        &self,
    ) -> (
        <MevBlocks as reth_db::table::Table>::Key,
        <MevBlocks as CompressedTable>::DecompressedValue,
    ) {
        (self.block_number, self.mev_blocks.clone())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(ClassifiedMev, SpecificMev)>,
}
