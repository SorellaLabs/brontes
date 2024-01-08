use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    impl_compress_decompress_for_serde,
};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use super::LibmdbxData;
use crate::tables::MevBlocks;

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
        (self.block_number, self.mev_blocks.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
}

impl_compress_decompress_for_serde!(MevBlockWithClassified);
