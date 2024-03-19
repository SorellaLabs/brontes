#![allow(non_camel_case_types)]
#![allow(private_bounds)]

use std::path::Path;

pub use brontes_types::db::traits::{DBWriter, LibmdbxReader};

pub mod initialize;
mod libmdbx_read_write;
use eyre::Context;
use implementation::compressed_wrappers::tx::CompressedLibmdbxTx;
use initialize::LibmdbxInitializer;
pub use libmdbx_read_write::{determine_eth_prices, LibmdbxInit, LibmdbxReadWriter};
use reth_db::{
    is_database_empty,
    mdbx::DatabaseArguments,
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::{RO, RW};
use tables::*;
use tracing::info;

use self::types::{CompressedTable, LibmdbxData};

pub mod implementation;
pub use implementation::compressed_wrappers::*;
pub mod tables;
pub mod types;
pub mod utils;

#[cfg(feature = "tests")]
pub mod test_utils;

#[derive(Debug)]
pub struct Libmdbx(DatabaseEnv);

impl Libmdbx {
    /// Opens up an existing database or creates a new one at the specified
    /// path. Creates tables if necessary. Opens in read/write mode.
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        let rpath = path.as_ref();
        if is_database_empty(rpath) {
            std::fs::create_dir_all(rpath).wrap_err_with(|| {
                format!("Could not create database directory {}", rpath.display())
            })?;
            // create_db_version_file(rpath)?;
        } else {
            match check_db_version_file(rpath) {
                Ok(_) => (),
                Err(DatabaseVersionError::MissingFile) => create_db_version_file(rpath)?,
                Err(err) => return Err(err.into()),
            }
        }

        let db = DatabaseEnv::open(
            rpath,
            DatabaseEnvKind::RW,
            DatabaseArguments::default().log_level(log_level),
        )?;

        let this = Self(db);
        this.create_tables()?;

        Ok(this)
    }

    /// Creates all the defined tables, opens if already created
    fn create_tables(&self) -> Result<(), DatabaseError> {
        let tx = CompressedLibmdbxTx::new_rw_tx(&self.0)?;

        for table in Tables::ALL {
            tx.0.create_table(&table)?;
        }

        tx.commit()?;

        Ok(())
    }

    /// Clears a table in the database
    pub fn clear_table<T>(&self) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        info!(target: "brontes::init", "{} -- Clearing Table", T::NAME);
        let tx = self.rw_tx()?;
        tx.clear::<T>()?;
        tx.commit()?;

        Ok(())
    }

    /// writes to a table
    pub fn write_table<T, D>(&self, entries: &[D]) -> Result<(), DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>,
    {
        self.update_db(|tx| {
            entries
                .iter()
                .map(|entry| {
                    let e = entry.into_key_val();
                    tx.put::<T>(e.key, e.value)
                })
                .collect::<Result<Vec<_>, DatabaseError>>()?;
            Ok::<(), DatabaseError>(())
        })??;

        Ok(())
    }

    /// Takes a function and passes a RW transaction
    /// makes sure it's committed at the end of execution
    pub fn update_db<F, R>(&self, f: F) -> Result<R, DatabaseError>
    where
        F: FnOnce(&CompressedLibmdbxTx<RW>) -> R,
    {
        let tx = self.rw_tx()?;

        let res = f(&tx);
        tx.commit()?;

        Ok(res)
    }

    /// returns a RO transaction
    pub fn ro_tx(&self) -> eyre::Result<CompressedLibmdbxTx<RO>> {
        let tx = CompressedLibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx)
    }

    /// returns a RW transaction
    fn rw_tx(&self) -> Result<CompressedLibmdbxTx<RW>, DatabaseError> {
        let tx = CompressedLibmdbxTx::new_rw_tx(&self.0)?;

        Ok(tx)
    }
}

/*
    /// gets all addresses that were initialized in a given block
    //TODO: Joe - implement a range function so that we don't have to loop through
    // the entire block range and can simply batch query
    pub fn protocols_created_at_block(
        &self,
        block_num: u64,
    ) -> eyre::Result<Vec<(Address, Protocol, Pair)>> {
        let tx = self.ro_tx()?;
        let binding_tx = self.ro_tx()?;
        let info_tx = self.ro_tx()?;

        let mut res = Vec::new();

        for addr in tx
            .get::<PoolCreationBlocks>(block_num)?
            .map(|i| i.0)
            .unwrap_or(vec![])
        {
            let Some(protocol) = binding_tx.get::<AddressToProtocol>(addr.to_source())? else {
                continue;
            };
            let Some(info) = info_tx.get::<AddressToTokens>(addr.to_source())? else {
                continue;
            };
            res.push((
                addr.to_source(),
                protocol,
                Pair(info.token0.to_source(), info.token1.to_source()),
            ));
        }

        Ok(res)
    }
*/
