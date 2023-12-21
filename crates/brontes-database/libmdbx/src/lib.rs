use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc};

use brontes_pricing::types::DexQuotes;

use crate::types::token_decimals::TokenDecimalsData;
pub mod initialize;

use alloy_primitives::Address;
use brontes_database::{clickhouse::Clickhouse, MetadataDB, Pair};
use brontes_pricing::types::DexPrices;
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    exchanges::StaticBindingsDb,
};
use eyre::Context;
use initialize::LibmdbxInitializer;
use malachite::Rational;
use reth_db::{
    cursor::DbCursorRO,
    is_database_empty,
    mdbx::DatabaseFlags,
    table::{DupSort, Table},
    transaction::{DbTx, DbTxMut},
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError, TableType,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::RO;
use tables::*;
use types::{cex_price::CexPriceMap, metadata::MetadataInner, LibmdbxDupData};

use self::{implementation::tx::LibmdbxTx, tables::Tables, types::LibmdbxData};
pub mod implementation;
pub mod tables;
pub mod types;

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
        let tx = LibmdbxTx::new_rw_tx(&self.0)?;

        for table in Tables::ALL {
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

    pub async fn clear_and_initialize_tables(
        &self,
        clickhouse: &Clickhouse,
        tables: &[Tables],
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse);
        initializer.initialize(tables, block_range).await?;

        Ok(())
    }

    pub fn try_get_decimals(&self, address: Address) -> Option<u8> {
        let db_tx = self.ro_tx().unwrap();
        db_tx.get::<TokenDecimals>(address).ok()?
    }

    /// Clears a table in the database
    /// Only called on initialization
    pub(crate) fn initialize_table<T, D>(&self, entries: &Vec<D>) -> eyre::Result<()>
    where
        T: Table,
        D: LibmdbxData<T>,
    {
        let tx = LibmdbxTx::new_rw_tx(&self.0)?;
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
        let tx = LibmdbxTx::new_rw_tx(&self.0)?;

        entries
            .into_iter()
            .map(|entry| {
                let (key, val) = entry.into_key_val();
                tx.put::<T>(key, val)
            })
            .collect::<Result<Vec<_>, _>>()?;

        tx.commit()?;

        Ok(())
    }

    /// returns a RO transaction
    pub fn ro_tx(&self) -> eyre::Result<LibmdbxTx<RO>> {
        let tx = LibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx)
    }

    pub fn insert_decimals(&self, address: Address, decimals: u8) -> eyre::Result<()> {
        self.write_table(&vec![TokenDecimalsData { address, decimals }])
    }

    pub fn addresses_inited_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<HashMap<(Address, StaticBindingsDb), Pair>> {
        let tx = self.ro_tx()?;
        let binding_tx = self.ro_tx()?;
        let info_tx = self.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(0..=block_num)? {
            let (_, res) = result?;
            for addr in res.0.into_iter() {
                let protocol = binding_tx.get::<AddressToProtocol>(addr)?.unwrap();
                let info = info_tx.get::<AddressToTokens>(addr)?.unwrap();
                map.insert((addr, protocol), Pair(info.token0, info.token1));
            }
        }

        Ok(map)
    }

    /// gets all addresses that were initialized in a given block
    pub fn addresses_init_block(
        &self,
        block_num: u64,
    ) -> eyre::Result<Vec<(Address, StaticBindingsDb, Pair)>> {
        let tx = self.ro_tx()?;
        let binding_tx = self.ro_tx()?;
        let info_tx = self.ro_tx()?;

        let mut res = Vec::new();

        for addr in tx.get::<PoolCreationBlocks>(block_num)?.unwrap().0 {
            let protocol = binding_tx.get::<AddressToProtocol>(addr)?.unwrap();
            let info = info_tx.get::<AddressToTokens>(addr)?.unwrap();
            res.push((addr, protocol, Pair(info.token0, info.token1)));
        }

        Ok(res)
    }

    pub fn get_metadata_no_dex(
        &self,
        block_num: u64,
    ) -> eyre::Result<brontes_database::MetadataDB> {
        let tx = LibmdbxTx::new_ro_tx(&self.0)?;
        let block_meta: MetadataInner = tx
            .get::<Metadata>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        // let cex_quotes: CexPriceMap = tx
        //     .get::<CexPrice>(block_num)?
        //     .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        Ok(MetadataDB {
            block_num,
            block_hash: block_meta.block_hash,
            relay_timestamp: block_meta.relay_timestamp,
            p2p_timestamp: block_meta.p2p_timestamp,
            proposer_fee_recipient: block_meta.proposer_fee_recipient, /* change this */
            proposer_mev_reward: block_meta.proposer_mev_reward,
            cex_quotes: brontes_database::cex::CexPriceMap::new(), /* brontes_database::cex::CexPriceMap(cex_quotes.0), // ambiguous type */
            eth_prices: Rational::default(),                       /* cex_quotes.0.get(&
                                                                    * Pair(Address::from_str("
                                                                    * ").unwrap(),
                                                                    * Address::from_str("").
                                                                    * unwrap())).unwrap() //
                                                                    * ambiguous type //
                                                                    * change to USDC - ETH +
                                                                    * error handle */
            mempool_flow: block_meta.mempool_flow.into_iter().collect(),
            block_timestamp: block_meta.block_timestamp,
        })
    }

    //TODO: Joe - implement
    pub fn get_metadata(&self, block_num: u64) -> eyre::Result<brontes_database::Metadata> {
        let tx = LibmdbxTx::new_ro_tx(&self.0)?;
        let block_meta: MetadataInner = tx
            .get::<Metadata>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        let cex_quotes: CexPriceMap = tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        //let eth_prices = ;

        let map = Arc::new(HashMap::new());

        Ok(brontes_database::Metadata {
            db:         MetadataDB {
                block_num,
                block_hash: block_meta.block_hash,
                relay_timestamp: block_meta.relay_timestamp,
                p2p_timestamp: block_meta.p2p_timestamp,
                proposer_fee_recipient: block_meta.proposer_fee_recipient, /* change this */
                proposer_mev_reward: block_meta.proposer_mev_reward,
                cex_quotes: brontes_database::cex::CexPriceMap::new(), /* brontes_database::cex::CexPriceMap(cex_quotes.0), // ambiguous type */
                eth_prices: Rational::default(),
                block_timestamp: block_meta.block_timestamp, /* cex_quotes.0.get(&
                                                              * Pair(Address::from_str("
                                                              * ").unwrap(),
                                                              * Address::from_str("").
                                                              * unwrap())).unwrap() //
                                                              * ambiguous type //
                                                              * change to USDC - ETH +
                                                              * error handle */
                mempool_flow: block_meta.mempool_flow.into_iter().collect(),
            },
            dex_quotes: DexPrices::new(map, DexQuotes(vec![])),
        })
    }

    pub fn insert_classified_data(
        &self,
        block_details: MevBlock,
        mev_details: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) {
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
