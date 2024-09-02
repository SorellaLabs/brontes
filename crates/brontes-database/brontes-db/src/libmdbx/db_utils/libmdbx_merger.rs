use std::path::PathBuf;

use brontes_types::BrontesTaskExecutor;
use fs_extra::dir::get_dir_content;
use indicatif::MultiProgress;
use rayon::iter::*;

use crate::{libmdbx::LibmdbxReadWriter, move_tables_to_partition, *};

pub fn merge_libmdbx_dbs(
    final_db: LibmdbxReadWriter,
    partition_db_folder: &PathBuf,
    executor: BrontesTaskExecutor,
) -> eyre::Result<()> {
    let files = get_dir_content(partition_db_folder)?;
    let multi = MultiProgress::default();
    // we can par this due to the single reader and not have any read locks.
    files
        .directories
        .par_iter()
        .filter(|dir_name| *dir_name != partition_db_folder.to_str().unwrap())
        .filter_map(|path| LibmdbxReadWriter::init_db(path, None, &executor, false).ok())
        .try_for_each(|db| {
            move_tables_to_partition!(FULL_RANGE db, final_db, Some(multi.clone()),
            CexPrice,
            CexTrades,
            BlockInfo,
            MevBlocks,
            InitializedState,
            PoolCreationBlocks,
            TxTraces,
            AddressMeta,
            SearcherEOAs,
            SearcherContracts,
            Builder,
            AddressToProtocolInfo,
            TokenDecimals,
            DexPrice
            );

            eyre::Ok(())
        })
}
