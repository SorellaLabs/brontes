use std::{
    path::PathBuf,
    sync::Arc,
    task::{Context, Waker},
    time::Duration,
};

use brontes_types::{db::dex::make_filter_key_range, BrontesTaskExecutor};
use futures::FutureExt;
use libmdbx::libmdbx_writer::InitTables;
use rayon::iter::*;
use tokio::sync::Notify;

use super::rclone_wrapper::BlockRangeList;
use crate::{
    libmdbx::{types::LibmdbxData, LibmdbxInit, LibmdbxReadWriter},
    CompressedTable, DexPrice, *,
};

pub const PARTITION_FILE_NAME: &str = "brontes-db-partition";

/// 1 week / 12 seconds
pub const DEFAULT_PARTITION_SIZE: u64 = 50_400;

#[macro_export]
macro_rules! move_tables_to_partition {
    (BLOCK_RANGE $parent_db:expr, $db:expr, $start_block:expr,$end_block:expr,
     $($table_name:ident),*) => {
        $(
            tracing::info!(start_block=%$start_block, end_block=%$end_block,
                           "loading data from table: {}", stringify!($table_name));
            ::paste::paste!(
                $parent_db.write_partition_range_data::<$table_name,
                [<$table_name Data>]>($start_block, $end_block, &$db)?;
            );
        )*
    };
    (FULL_RANGE $parent_db:expr, $db:expr, $($table_name:ident),*) => {
        $(
            tracing::info!("loading data from table: {}", stringify!($table_name));
            ::paste::paste!(
                $parent_db.write_critical_data::
                <$table_name, [<$table_name Data>]>(&$db)?;
            );
        )*
    }
}

pub struct LibmdbxPartitioner {
    // db with all the data
    parent_db:           LibmdbxReadWriter,
    partition_db_folder: PathBuf,
    start_block:         u64,
    executor:            BrontesTaskExecutor,
}

impl LibmdbxPartitioner {
    pub fn new(
        parent_db: LibmdbxReadWriter,
        partition_db_folder: PathBuf,
        start_block: u64,
        executor: BrontesTaskExecutor,
    ) -> Self {
        fs_extra::dir::create_all(&partition_db_folder, false)
            .expect("failed to create partition db folder");

        Self { parent_db, start_block, partition_db_folder, executor }
    }

    pub fn execute(self) -> eyre::Result<()> {
        // cleanup
        let mut start_block = self.start_block;
        let end_block = self.parent_db.get_db_range()?.1;

        let mut ranges = vec![];
        while start_block + DEFAULT_PARTITION_SIZE < end_block {
            ranges.push(BlockRangeList {
                start_block,
                end_block: start_block + DEFAULT_PARTITION_SIZE,
            });

            start_block += DEFAULT_PARTITION_SIZE
        }
        tracing::info!(?ranges, "partitioning db into ranges");

        // because we are just doing read operations. we can do all this in parallel
        ranges
            .par_iter()
            .try_for_each(|BlockRangeList { start_block, end_block }| {
                let mut path = self.partition_db_folder.clone();
                path.push(format!("{PARTITION_FILE_NAME}-{start_block}-{end_block}/"));
                tracing::info!(?path, "creating path");
                fs_extra::dir::create_all(&path, false)?;
                let db = LibmdbxReadWriter::init_db(path, None, &self.executor, false)?;
                tracing::info!("database opened");

                move_tables_to_partition!(
                    BLOCK_RANGE
                    self.parent_db,
                    db,
                    *start_block,
                    *end_block,
                    CexPrice,
                    CexTrades,
                    BlockInfo,
                    MevBlocks,
                    InitializedState,
                    PoolCreationBlocks,
                    TxTraces
                );
                // manually dex pricing
                self.parent_db
                    .write_dex_price_range(*start_block, *end_block, &db)
            })?;

        // move over full range tables
        let mut path = self.partition_db_folder.clone();
        path.push(format!("{PARTITION_FILE_NAME}-full-range-tables/",));
        fs_extra::dir::create_all(&path, false)?;
        let db = LibmdbxReadWriter::init_db(path, None, &self.executor, false)?;

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
}

impl LibmdbxReadWriter {
    pub fn write_partition_range_data<T, D>(
        &self,
        start_block: u64,
        end_block: u64,
        write_db: &LibmdbxReadWriter,
    ) -> eyre::Result<()>
    where
        T: CompressedTable<Key = u64>,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
        InitTables: From<Vec<D>>,
    {
        let tx = self.db.no_timeout_ro_tx()?;
        let mut cur = tx.cursor_read::<T>()?;

        TmpWriter::<T, D>::batch_write_to_db(
            cur.walk_range(start_block..end_block)?
                .flatten()
                .map(|value| (value.0, value.1)),
            write_db,
            500,
        );
        Ok(())
    }

    // dex table has special key
    pub fn write_dex_price_range(
        &self,
        start_block: u64,
        end_block: u64,
        write_db: &LibmdbxReadWriter,
    ) -> eyre::Result<()> {
        let tx = self.db.no_timeout_ro_tx()?;
        let mut cur = tx.cursor_read::<DexPrice>()?;

        let start_key = make_filter_key_range(start_block).0;
        let end_key = make_filter_key_range(end_block).1;

        TmpWriter::<DexPrice, DexPriceData>::batch_write_to_db(
            cur.walk_range(start_key..end_key)?
                .flatten()
                .map(|value| (value.0, value.1)),
            write_db,
            500,
        );

        Ok(())
    }

    pub fn write_critical_data<T, D>(&self, write_db: &LibmdbxReadWriter) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
        InitTables: From<Vec<D>>,
    {
        let tx = self.db.no_timeout_ro_tx()?;
        let mut cur = tx.cursor_read::<T>()?;

        TmpWriter::<T, D>::batch_write_to_db(
            cur.walk(None)?.flatten().map(|val| (val.0, val.1)),
            write_db,
            500,
        );

        Ok(())
    }

    pub fn write_partitioned_range_data<T, D>(
        &self,
        data: Vec<(T::Key, T::DecompressedValue)>,
    ) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
        InitTables: From<Vec<D>>,
    {
        let mapped = data.into_iter().map(D::from).collect::<Vec<_>>();
        let not = Arc::new(Notify::new());
        self.tx
            .send(libmdbx::libmdbx_writer::WriterMessage::Init(mapped.into(), not.clone()))?;

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut no = not.notified();
        let mut pinned = std::pin::pin!(no);
        loop {
            if pinned.poll_unpin(&mut cx).is_ready() {
                break
            }

            std::thread::sleep(Duration::from_micros(250));
        }

        Ok(())
    }
}

impl<I: Sized, T, D> TmpWriter<T, D> for I
where
    I: Iterator<Item = (T::Key, T::DecompressedValue)>,
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
{
}

pub trait TmpWriter<T, D>: Iterator<Item = (T::Key, T::DecompressedValue)>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + From<(T::Key, T::DecompressedValue)>,
{
    fn batch_write_to_db(self, db: &LibmdbxReadWriter, batch_size: usize)
    where
        Self: Sized,
        InitTables: From<Vec<D>>,
    {
        let mut batch = Vec::with_capacity(batch_size);
        for next in self {
            batch.push(next);
            if batch.len() == batch_size {
                db.write_partitioned_range_data::<T, D>(std::mem::take(&mut batch))
                    .expect("failed to write partitioned data");
            }
        }

        // write final amount that wasn't batched
        db.write_partitioned_range_data::<T, D>(batch)
            .expect("failed to write partitioned data");
    }
}
