use std::path::PathBuf;

use brontes_types::BrontesTaskExecutor;
use fs_extra::dir::get_dir_content;

use super::LibmdbxReadWriter;
use crate::{
    libmdbx::libmdbx_partitioning::PARTITION_FILE_NAME, move_tables_to_partition, DexPrice, *,
};

pub fn merge_libmdbx_dbs(
    final_db: LibmdbxReadWriter,
    partition_db_folder: PathBuf,
    executor: BrontesTaskExecutor,
) -> eyre::Result<()> {
    let files = get_dir_content(&partition_db_folder)?;
    let dbs = files
        .files
        .iter()
        .filter(|file_name| file_name.starts_with(PARTITION_FILE_NAME))
        .filter_map(|path| LibmdbxReadWriter::init_db(path, None, &executor).ok())
        .collect::<Vec<_>>();

    for db in dbs {
        move_tables_to_partition!(FULL_RANGE db, final_db,
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
    }

    Ok(())
}
