#![allow(non_camel_case_types)]
#![allow(private_bounds)]

use std::{collections::HashMap, path::Path, sync::Arc};

pub mod initialize;

use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
use brontes_types::extra_processing::Pair;
use eyre::Context;
use implementation::compressed_wrappers::tx::CompressedLibmdbxTx;
use initialize::LibmdbxInitializer;
use reth_db::{
    is_database_empty,
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::{RO, RW};
use tables::*;
use tracing::info;

use self::types::{subgraphs::SubGraphsData, CompressedTable, LibmdbxData};
use crate::clickhouse::Clickhouse;

pub mod implementation;
pub use implementation::compressed_wrappers::*;
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
        let tx = CompressedLibmdbxTx::new_rw_tx(&self.0)?;

        for table in Tables::ALL {
            tx.0.create_table(&table)?;
        }

        tx.commit()?;

        Ok(())
    }

    /// initializes all the tables with data via the CLI
    pub async fn initialize_tables(
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
    fn clear_table<T>(&self) -> eyre::Result<()>
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
    pub fn write_table<T, D>(&self, entries: &Vec<D>) -> Result<(), DatabaseError>
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

    /// idk
    pub fn protocols_created_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<HashMap<(Address, Protocol), Pair>> {
        let tx = self.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(0..=block_num)? {
            let res = result?.1;
            for addr in res.0.into_iter() {
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.insert((addr, protocol), Pair(info.token0, info.token1));
            }
        }

        info!(target:"brontes-libmdbx", "loaded {} pairs before block: {}", map.len(), block_num);

        Ok(map)
    }

    /// idk
    pub fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<HashMap<u64, Vec<(Address, Protocol, Pair)>>> {
        let tx = self.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(start_block..end_block)? {
            let result = result?;
            let (block, res) = (result.0, result.1);
            for addr in res.0.into_iter() {
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.entry(block).or_insert(vec![]).push((
                    addr,
                    protocol,
                    Pair(info.token0, info.token1),
                ));
            }
        }
        info!(target:"brontes-libmdbx", "loaded {} pairs range: {}..{}", map.len(), start_block, end_block);

        Ok(map)
    }

    /// idl
    pub fn save_pair_at(
        &self,
        block: u64,
        pair: Pair,
        edges: Vec<SubGraphEdge>,
    ) -> eyre::Result<()> {
        let tx = self.ro_tx()?;
        if let Some(mut entry) = tx.get::<SubGraphs>(pair)? {
            entry.0.insert(block, edges.into_iter().collect::<Vec<_>>());

            let data = SubGraphsData { pair, data: entry };
            self.write_table::<SubGraphs, SubGraphsData>(&vec![data])?;
        }

        Ok(())
    }

    /// idl
    pub fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        let tx = self.ro_tx()?;
        let subgraphs = tx
            .get::<SubGraphs>(pair)?
            .ok_or_else(|| eyre::eyre!("no subgraph found"))?;

        // load the latest version of the sub graph relative to the block. if the
        // sub graph is the last entry in the vector, we return an error as we cannot
        // grantee that we have a run from last update to request block
        let last_block = *subgraphs.0.keys().max().unwrap();
        if block > last_block {
            eyre::bail!("possible missing state");
        }

        let mut last: Option<(Pair, Vec<SubGraphEdge>)> = None;

        for (cur_block, update) in subgraphs.0 {
            if cur_block > block {
                return last.ok_or_else(|| eyre::eyre!("no subgraph found"))
            }
            last = Some((pair, update))
        }

        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use serial_test::serial;

    use crate::libmdbx::Libmdbx;

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
