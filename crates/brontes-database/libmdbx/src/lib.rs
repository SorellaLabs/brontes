use std::{cmp::max, collections::HashMap, path::Path, str::FromStr, sync::Arc};

use brontes_pricing::SubGraphEdge;
use brontes_pricing::types::DexQuotes;
pub mod initialize;

use alloy_primitives::Address;
use brontes_database::{clickhouse::Clickhouse, MetadataDB, Pair};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    exchanges::StaticBindingsDb,
};
use eyre::Context;
use initialize::LibmdbxInitializer;
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
pub use tables::*;
use tracing::info;
use types::{
    cex_price::CexPriceMap,
    dex_price::{make_filter_key_range, DexPriceData},
    metadata::MetadataInner,
    mev_block::{MevBlockWithClassified, MevBlocksData},
    token_decimals::TokenDecimalsData,
};

use self::{implementation::tx::LibmdbxTx, types::LibmdbxData};
pub mod implementation;
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

    pub fn insert_quotes(&self, block_num: u64, quotes: DexQuotes) -> eyre::Result<()> {
        let mut data = quotes
            .0
            .into_iter()
            .enumerate()
            .filter(|(_, v)| v.is_some())
            .map(|(idx, value)| DexPriceData {
                block_number: block_num,
                tx_idx:       idx as u16,
                quote:        types::dex_price::DexQuote(value.unwrap()),
            })
            .collect::<Vec<_>>();

        data.sort_by(|a, b| a.tx_idx.cmp(&b.tx_idx));
        data.sort_by(|a, b| a.block_number.cmp(&b.block_number));

        let tx = LibmdbxTx::new_rw_tx(&self.0)?;
        let mut cursor = tx.cursor_write::<DexPrice>()?;

        data.into_iter()
            .map(|entry| {
                let (key, val) = entry.into_key_val();
                //let key = make_key(key., val.tx_idx);

                cursor.upsert(key, val)?;
                Ok(())
            })
            .collect::<Result<Vec<_>, DatabaseError>>()?;

        tx.commit()?;

        //self.write_table::<DexPrice, DexPriceData>(&data)?;
        Ok(())
    }

    pub async fn clear_and_initialize_tables(
        self: Arc<Self>,
        clickhouse: Arc<Clickhouse>,
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

    pub fn try_load_pair_before(&self, block: u64, pair: Pair) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        todo!()
    }

    pub fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()> {
        todo!()
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
            .iter()
            .map(|entry| {
                let (key, val) = entry.into_key_val();
                tx.put::<T>(key, val)
            })
            .collect::<Result<Vec<_>, _>>()?;

        tx.commit()?;

        Ok(())
    }

    /// Clears a table in the database
    /// Only called on initialization
    pub(crate) fn clear_table<T>(&self) -> eyre::Result<()>
    where
        T: Table,
    {
        let tx = LibmdbxTx::new_rw_tx(&self.0)?;
        tx.clear::<T>()?;
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
                let Some(protocol) = binding_tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = info_tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.insert((addr, protocol), Pair(info.token0, info.token1));
            }
        }

        info!(target:"brontes-libmdbx", "loaded {} pairs before block: {}", map.len(), block_num);

        Ok(map)
    }

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
            let Some(protocol) = binding_tx.get::<AddressToProtocol>(addr)? else {
                continue;
            };
            let Some(info) = info_tx.get::<AddressToTokens>(addr)? else {
                continue;
            };
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
        let db_cex_quotes: CexPriceMap = tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        let eth_prices = if let Some(eth_usdt) = db_cex_quotes.get_quote(&Pair(
            Address::from_str(WETH_ADDRESS).unwrap(),
            Address::from_str(USDT_ADDRESS).unwrap(),
        )) {
            eth_usdt
        } else {
            db_cex_quotes
                .get_quote(&Pair(
                    Address::from_str(WETH_ADDRESS).unwrap(),
                    Address::from_str(USDC_ADDRESS).unwrap(),
                ))
                .unwrap_or_default()
        };

        let mut cex_quotes = brontes_database::cex::CexPriceMap::new();
        db_cex_quotes.0.into_iter().for_each(|(pair, quote)| {
            cex_quotes.0.insert(
                pair,
                quote
                    .into_iter()
                    .map(|q| brontes_database::cex::CexQuote {
                        exchange:  q.exchange,
                        timestamp: q.timestamp,
                        price:     q.price,
                        token0:    q.token0,
                    })
                    .collect::<Vec<_>>(),
            );
        });

        Ok(MetadataDB {
            block_num,
            block_hash: block_meta.block_hash,
            relay_timestamp: block_meta.relay_timestamp,
            p2p_timestamp: block_meta.p2p_timestamp,
            proposer_fee_recipient: block_meta.proposer_fee_recipient,
            proposer_mev_reward: block_meta.proposer_mev_reward,
            cex_quotes,
            eth_prices: max(eth_prices.price.0, eth_prices.price.1),

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
        let db_cex_quotes: CexPriceMap = tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        let eth_prices = if let Some(eth_usdt) = db_cex_quotes.get_quote(&Pair(
            Address::from_str(WETH_ADDRESS).unwrap(),
            Address::from_str(USDT_ADDRESS).unwrap(),
        )) {
            eth_usdt
        } else {
            db_cex_quotes
                .get_quote(&Pair(
                    Address::from_str(WETH_ADDRESS).unwrap(),
                    Address::from_str(USDC_ADDRESS).unwrap(),
                ))
                .unwrap_or_default()
        };

        let mut cex_quotes = brontes_database::cex::CexPriceMap::new();
        db_cex_quotes.0.into_iter().for_each(|(pair, quote)| {
            cex_quotes.0.insert(
                pair,
                quote
                    .into_iter()
                    .map(|q| brontes_database::cex::CexQuote {
                        exchange:  q.exchange,
                        timestamp: q.timestamp,
                        price:     q.price,
                        token0:    q.token0,
                    })
                    .collect::<Vec<_>>(),
            );
        });

        let dex_quotes = Vec::new();
        let key_range = make_filter_key_range(block_num);
        let _db_dex_quotes = tx
            .cursor_read::<DexPrice>()?
            .walk_range(key_range.0..key_range.1)?
            .flat_map(|inner| {
                if let Ok((key, _quote)) = inner {
                    //dex_quotes.push(Default::default());
                    Some(key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        //.get::<DexPrice>(block_num)?
        //.ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        Ok(brontes_database::Metadata {
            db:         MetadataDB {
                block_num,
                block_hash: block_meta.block_hash,
                relay_timestamp: block_meta.relay_timestamp,
                p2p_timestamp: block_meta.p2p_timestamp,
                proposer_fee_recipient: block_meta.proposer_fee_recipient,
                proposer_mev_reward: block_meta.proposer_mev_reward,
                cex_quotes,
                eth_prices: max(eth_prices.price.0, eth_prices.price.1),
                block_timestamp: block_meta.block_timestamp,
                mempool_flow: block_meta.mempool_flow.into_iter().collect(),
            },
            dex_quotes: DexQuotes(dex_quotes),
        })
    }

    pub fn insert_classified_data(
        &self,
        block: MevBlock,
        mev: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) -> eyre::Result<()> {
        self.write_table(&vec![MevBlocksData {
            block_number: block.block_number,
            mev_blocks:   MevBlockWithClassified { block, mev },
        }])
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
