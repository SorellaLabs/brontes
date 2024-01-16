pub mod cursor;
pub mod tx;
mod utils;

use std::{cmp::max, collections::HashMap, path::Path, str::FromStr, sync::Arc};

use alloy_primitives::Address;
use brontes_database::{clickhouse::Clickhouse, MetadataDB, Pair};
use brontes_database_libmdbx::types::LibmdbxData;
use brontes_pricing::types::{DexPrices, DexQuotes};
use eyre::Context;
use reth_db::{
    cursor::{DbCursorRO, DbCursorRW},
    is_database_empty,
    mdbx::DatabaseFlags,
    table::Table,
    transaction::{DbTx, DbTxMut},
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError, TableType,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::RO;
use reth_tracing_ext::TracingClient;

use self::tx::LibmdbxTxBench;
use crate::setup::tables::BenchTables;

#[derive(Debug)]
pub struct LibmdbxBench(DatabaseEnv);

impl LibmdbxBench {
    /// Opens up an existing database or creates a new one at the specified
    /// path. Creates tables if necessary. Opens in read/write mode.
    pub fn init_db<P: AsRef<Path>>(
        path: P,
        tables: &[BenchTables],
        log_level: Option<LogLevel>,
    ) -> eyre::Result<Self> {
        let rpath = path.as_ref();
        if is_database_empty(rpath) {
            std::fs::create_dir_all(rpath).wrap_err_with(|| {
                format!("Could not create database directory {}", rpath.display())
            })?;
            //create_db_version_file(rpath)?;
        } else {
            match check_db_version_file(rpath) {
                Ok(_) => (),
                Err(DatabaseVersionError::MissingFile) => create_db_version_file(rpath)?,
                Err(err) => return Err(err.into()),
            }
        }

        let db = DatabaseEnv::open(rpath, DatabaseEnvKind::RW, log_level)?;

        let this = Self(db);
        this.create_tables(tables)?;

        Ok(this)
    }

    /// Creates all the defined tables, opens if already created
    fn create_tables(&self, tables: &[BenchTables]) -> Result<(), DatabaseError> {
        let tx = LibmdbxTxBench::new_rw_tx(&self.0)?;

        for table in tables {
            let flags = match table.table_type() {
                TableType::Table => DatabaseFlags::default(),
                TableType::DupSort => DatabaseFlags::DUP_SORT,
            };

            tx.inner
                .create_db(Some(table.name()), flags)
                .map_err(|e| DatabaseError::CreateTable(e.into()))?;
        }

        tx.commit()?;

        Ok(())
    }

    /// Clears a table in the database
    /// Only called on initialization
    pub fn initialize_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: Table,
        D: LibmdbxData<T>,
    {
        let tx = LibmdbxTxBench::new_rw_tx(&self.0)?;
        tx.clear::<T>()?;
        tx.commit()?;

        self.write_table(entries)?;

        Ok(())
    }

    /// writes to a table
    pub fn write_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: Table,
        D: LibmdbxData<T>,
    {
        let tx = LibmdbxTxBench::new_rw_tx(&self.0)?;

        entries
            .iter()
            .map(|entry| {
                let (key, val) = entry.into_key_val();
                tx.put::<T>(key, val)
            })
            .collect::<Result<Vec<_>, _>>()?;

        tx.commit()?;

        Ok(())
    }

    pub fn size_of_table<T: Table>(&self) -> usize {
        let tx = LibmdbxTxBench::new_ro_tx(&self.0).unwrap();

        let table_db = tx
            .inner
            .open_db(Some(T::NAME))
            .wrap_err("Could not open db.")
            .unwrap();
        let stats = tx.inner.db_stat(&table_db).unwrap();

        let page_size = stats.page_size() as usize;
        let leaf_pages = stats.leaf_pages();
        let branch_pages = stats.branch_pages();
        let overflow_pages = stats.overflow_pages();
        let num_pages = leaf_pages + branch_pages + overflow_pages;
        let table_size = page_size * num_pages;

        table_size
    }

    pub fn bench_read_full_table<T: Table>(&self, group_name: &str) {
        let tx = LibmdbxTxBench::new_ro_tx(&self.0).unwrap();

        let mut cursor = tx.cursor_read::<T>().unwrap();

        let vals = cursor
            .walk_range(..)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // println!("{} - FOUND VALS: {}", group_name, vals.len())
    }

    pub fn bench_write_full_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: Table,
        D: LibmdbxData<T>,
    {
        let tx = LibmdbxTxBench::new_rw_tx(&self.0)?;
        tx.clear::<T>()?;
        tx.commit()?;

        self.write_table(entries)?;

        Ok(())
    }
}
