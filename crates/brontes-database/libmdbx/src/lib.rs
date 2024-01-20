#![allow(non_camel_case_types)]
#![allow(private_bounds)]
use std::{cmp::max, collections::HashMap, path::Path, str::FromStr, sync::Arc};

use brontes_pricing::{types::DexQuotes, SubGraphEdge};
pub mod initialize;

use alloy_primitives::Address;
use brontes_database::{clickhouse::Clickhouse, MetadataDB, Pair};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    exchanges::StaticBindingsDb,
    libmdbx::redefined_types::primitives::Redefined_Address,
};
use eyre::Context;
use implementation::compressed_wrappers::tx::CompressedLibmdbxTx;
use initialize::LibmdbxInitializer;
use redefined::RedefinedConvert;
use reth_db::{
    is_database_empty,
    table::Table,
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::{RO, RW};
use tables::*;
use tracing::info;
use types::{
    address_to_protocol::AddressToProtocolData,
    address_to_tokens::{AddressToTokensData, PoolTokens},
    cex_price::CexPriceMap,
    dex_price::{make_filter_key_range, DexPriceData},
    metadata::MetadataInner,
    mev_block::{MevBlockWithClassified, MevBlocksData},
    pool_creation_block::PoolCreationBlocksData,
    redefined_types::subgraph::Redefined_SubGraphEdge,
    token_decimals::TokenDecimalsData,
};

use self::types::LibmdbxData;
use crate::types::subgraphs::SubGraphsData;
mod implementation;
pub use implementation::compressed_wrappers::*;
pub mod tables;
pub mod types;

const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDT_ADDRESS: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
//const USDT_ADDRESS: &str = ;

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

    /// initializes all the tables with data via the CLI
    pub async fn init_tables(
        self: Arc<Self>,
        clickhouse: Arc<Clickhouse>,
        //tracer: Arc<TracingClient>,
        tables: &[Tables],
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse); //, tracer);
        initializer.initialize(tables, block_range).await?;

        Ok(())
    }

    /// Clears a table in the database
    /// Only called on initialization
    pub(crate) fn initialize_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>,
    {
        self.clear_table::<T>()?;
        self.write_table(entries)?;

        Ok(())
    }

    /// Clears a table in the database
    /// Only called on initialization
    fn clear_table<T>(&self) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let tx = self.rw_tx()?;
        tx.clear::<T>()?;
        tx.commit()?;

        Ok(())
    }

    /// writes to a table
    pub fn write_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>,
    {
        self.update_db(|tx| {
            entries
                .iter()
                .map(|entry| {
                    let (key, val) = entry.into_key_val();
                    tx.put::<T>(key, val)
                })
                .collect::<Result<Vec<_>, DatabaseError>>()?;
            Ok::<(), DatabaseError>(())
        })??;

        Ok(())
    }

    /// Takes a function and passes a RW transaction
    /// makes sure it's committed at the end of exec
    /// cause u heads r degens and have not used 'tx.commit()?;'
    /// once in this entire repo
    pub fn view_db<F, R>(&self, f: F) -> eyre::Result<R>
    where
        F: FnOnce(&CompressedLibmdbxTx<RO>) -> R,
    {
        let tx = self.ro_tx()?;

        let res = f(&tx);
        tx.commit()?;

        Ok(res)
    }

    /// Takes a function and passes a RW transaction
    /// makes sure it's committed at the end of exec
    /// cause u heads r degens and have not used 'tx.commit()?;'
    /// once in this entire repo, so i'm making all RW functionality private
    pub fn update_db<F, R>(&self, f: F) -> eyre::Result<R>
    where
        F: FnOnce(&CompressedLibmdbxTx<RW>) -> R,
    {
        let tx = self.rw_tx()?;

        let res = f(&tx);
        tx.commit()?;

        Ok(res)
    }

    /// returns a RO transaction
    fn ro_tx(&self) -> eyre::Result<CompressedLibmdbxTx<RO>> {
        let tx = CompressedLibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx)
    }

    /// returns a RO transaction
    pub fn temp_ro_tx(&self) -> eyre::Result<CompressedLibmdbxTx<RO>> {
        let tx = CompressedLibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx)
    }

    /// returns a RW transaction
    fn rw_tx(&self) -> eyre::Result<CompressedLibmdbxTx<RW>> {
        let tx = CompressedLibmdbxTx::new_rw_tx(&self.0)?;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use serial_test::serial;

    use crate::Libmdbx;

    fn init_db() -> eyre::Result<Libmdbx> {
        dotenv::dotenv().ok();
        let brontes_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        Libmdbx::init_db(brontes_db_path, None)
    }

    #[tokio::test]
    #[serial]
    async fn test_init_db() {
        init_db().unwrap();
        assert!(init_db().is_ok());
    }
}

/*
    /// gets all addresses that were initialized in a given block
    //TODO: Joe - implement a range function so that we don't have to loop through
    // the entire block range and can simply batch query
    pub fn protocols_created_at_block(
        &self,
        block_num: u64,
    ) -> eyre::Result<Vec<(Address, StaticBindingsDb, Pair)>> {
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
