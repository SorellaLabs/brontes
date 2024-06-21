use std::path::PathBuf;

use super::LibmdbxReadWriter;

const PARTION_NAME: &str = "brontes-db-partition";

pub struct LibmdbxPartitioner {
    // db with all the data
    parent_db:             LibmdbxReadWriter,
    partition_db_folder:   PathBuf,
    partition_size_blocks: usize,
    start_block:           Option<u64>,
}

impl LibmdbxPartitioner {
    pub fn new(
        parent_db: LibmdbxReadWriter,
        partition_db_folder: PathBuf,
        partition_size_blocks: usize,
        start_block: Option<u64>,
    ) -> Self {
        Self { parent_db, partition_size_blocks, start_block, partition_db_folder }
    }

    pub fn execute(self) {}

    fn check_most_recent_partition(&self) -> Option<u64> {}
}
