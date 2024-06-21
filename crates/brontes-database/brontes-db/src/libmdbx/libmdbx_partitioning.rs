use std::{path::PathBuf, str::FromStr};

use brontes_types::{
    db::dex::{make_filter_key_range, DexKey, DexQuoteWithIndex},
    BrontesTaskExecutor,
};
use fs_extra::dir::get_dir_content;
use rayon::iter::*;

use super::{types::LibmdbxData, LibmdbxReadWriter};
use crate::{libmdbx::LibmdbxInit, CompressedTable, DexPrice, *};

pub const PARTITION_FILE_NAME: &str = "brontes-db-partition";
/// 1 week / 12 seconds
const DEFAULT_PARTITION_SIZE: u64 = 50_400;

#[macro_export]
macro_rules! move_tables_to_partition {
    (BLOCK_RANGE $parent_db:expr, $db:expr, $start_block:expr,$end_block:expr,$($table_name:ident),*) => {
        $(
            let value = $parent_db.fetch_partition_range_data::<$table_name>($start_block, $end_block)?;
            ::paste::paste!(
                $db.write_partitioned_range_data::<$table_name, [<$table_name Data>]>(value)?;
            );
        )*
    };
    (FULL_RANGE $parent_db:expr, $db:expr, $($table_name:ident),*) => {
        $(
            let value = $parent_db.fetch_critical_data::<$table_name>()?;
            ::paste::paste!(
                $db.write_partitioned_range_data::<$table_name, [<$table_name Data>]>(value)?;
            );
        )*
    }
}

pub struct LibmdbxPartitioner {
    // db with all the data
    parent_db:             LibmdbxReadWriter,
    partition_db_folder:   PathBuf,
    partition_size_blocks: u64,
    start_block:           Option<u64>,
    executor:              BrontesTaskExecutor,
}

impl LibmdbxPartitioner {
    pub fn new(
        parent_db: LibmdbxReadWriter,
        partition_db_folder: PathBuf,
        partition_size_blocks: u64,
        start_block: Option<u64>,
        executor: BrontesTaskExecutor,
    ) -> Self {
        Self { parent_db, partition_size_blocks, start_block, partition_db_folder, executor }
    }

    pub fn execute(self) -> eyre::Result<()> {
        let mut start_block = self
            .start_block
            .or_else(|| self.check_most_recent_partition())
            .unwrap_or(0);

        let end_block = self.parent_db.get_db_range()?.1;

        let mut ranges = vec![];
        while start_block + self.partition_size_blocks < end_block {
            ranges.push((start_block, start_block + self.partition_size_blocks));
            start_block += self.partition_size_blocks
        }

        // because we are just doing read operations. we can do all this in parallel
        ranges
            .into_par_iter()
            .try_for_each(|(start_block, end_block)| {
                let mut path = self.partition_db_folder.clone();
                path.push(format!("{PARTITION_FILE_NAME}-{start_block}-{end_block}/"));
                fs_extra::dir::create_all(&path, false)?;
                let db = LibmdbxReadWriter::init_db(path, None, &self.executor)?;

                move_tables_to_partition!(
                    BLOCK_RANGE
                    self.parent_db,
                    db,
                    start_block,
                    end_block,
                    CexPrice,
                    CexTrades,
                    BlockInfo,
                    MevBlocks,
                    InitializedState,
                    PoolCreationBlocks,
                    TxTraces
                );
                // manually dex pricing
                let value = self
                    .parent_db
                    .fetch_dex_price_range(start_block, end_block)?;
                db.write_partitioned_range_data::<DexPrice, DexPriceData>(value)
            })?;

        // move over full range tables
        let mut path = self.partition_db_folder.clone();
        path.push(format!("{PARTITION_FILE_NAME}-full-range-tables/",));
        fs_extra::dir::create_all(&path, false)?;
        let db = LibmdbxReadWriter::init_db(path, None, &self.executor)?;

        move_tables_to_partition!(
            FULL_RANGE
            self.parent_db,
            db,
            AddressMeta,
            SearcherEOAs,
            SearcherContracts,
            Builder,
            AddressToProtocolInfo,
            TokenDecimals
        );

        Ok(())
    }

    fn check_most_recent_partition(&self) -> Option<u64> {
        let dir_content = get_dir_content(&self.partition_db_folder).ok()?;
        dir_content
            .files
            .iter()
            .filter(|file_name| file_name.starts_with(PARTITION_FILE_NAME))
            .filter_map(|files| u64::from_str(files.split('-').last()?.split('.').next()?).ok())
            .max()
    }
}

impl LibmdbxReadWriter {
    pub fn write_partitioned_range_data<T, D>(
        &self,
        data: Vec<(T::Key, T::DecompressedValue)>,
    ) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
    {
        let mapped = data.into_iter().map(D::from).collect::<Vec<_>>();
        self.db.write_table(&mapped)?;

        Ok(())
    }

    pub fn fetch_partition_range_data<T>(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<(T::Key, T::DecompressedValue)>>
    where
        T: CompressedTable<Key = u64>,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let tx = self.db.ro_tx()?;
        let mut cur = tx.cursor_read::<T>()?;
        Ok(cur
            .walk_range(start_block..end_block)?
            .into_iter()
            .flatten()
            .map(|value| (value.0, value.1))
            .collect::<Vec<_>>())
    }

    // dex table has special key
    pub fn fetch_dex_price_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<(DexKey, DexQuoteWithIndex)>> {
        let tx = self.db.ro_tx()?;
        let mut cur = tx.cursor_read::<DexPrice>()?;

        let start_key = make_filter_key_range(start_block).0;
        let end_key = make_filter_key_range(end_block).1;

        Ok(cur
            .walk_range(start_key..end_key)?
            .into_iter()
            .flatten()
            .map(|value| (value.0, value.1))
            .collect::<Vec<_>>())
    }

    pub fn fetch_critical_data<T>(&self) -> eyre::Result<Vec<(T::Key, T::DecompressedValue)>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let tx = self.db.ro_tx()?;
        let mut cur = tx.cursor_read::<T>()?;
        let mut res = vec![];
        while let Some(val) = cur.next()? {
            res.push((val.0, val.1));
        }
        Ok(res)
    }
}
