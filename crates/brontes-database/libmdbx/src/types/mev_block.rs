use brontes_types::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use super::LibmdbxData;
use crate::tables::MevBlocks;

/// there is no Serialize / Deserialize as this is done manually for Libmdbx.
/// if we are inserting into Clickhouse, use the library that downcasts the dyn
/// for inserts and decoding
#[derive(Debug, Row)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev:   Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
}

impl reth_db::table::Compress for MevBlockWithClassified {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {}
}
