use alloy_primitives::Address;
use brontes_types::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use super::LibmdbxData;
use crate::tables::MevBlocks;

//#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Row)]
pub struct MevBlocksData {
    pub block_number: u64,
    pub mev_blocks:   MevBlockWithClassified,
}

impl LibmdbxData<MevBlocks> for MevBlocksData {
    fn into_key_val(
        &self,
    ) -> (<MevBlocks as reth_db::table::Table>::Key, <MevBlocks as reth_db::table::Table>::Value)
    {
        (self.block_number, self.mev_blocks)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
}
