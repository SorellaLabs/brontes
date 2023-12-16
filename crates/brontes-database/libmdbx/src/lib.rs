use std::path::Path;
pub mod initialize;
use brontes_database::clickhouse::Clickhouse;
use eyre::Context;
use initialize::LibmdbxInitializer;
use reth_db::{
    is_database_empty,
    mdbx::DatabaseFlags,
    table::Table,
    transaction::{DbTx, DbTxMut},
    version::{check_db_version_file, create_db_version_file, DatabaseVersionError},
    DatabaseEnv, DatabaseEnvKind, DatabaseError, TableType,
};
use reth_interfaces::db::LogLevel;
use reth_libmdbx::RO;

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
    ) -> eyre::Result<()> {
        let initializer = LibmdbxInitializer::new(self, clickhouse);
        initializer.initialize(tables).await?;

        Ok(())
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

    /// reads a single value from a table
    pub fn get_table_one<T>(&self, key: &T::Key) -> eyre::Result<Option<T::Value>>
    where
        T: Table,
        T::Key: Copy,
    {
        let tx = LibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx.get::<T>(*key)?)
    }

    /// returns a RO transaction
    pub fn ro_tx(&self) -> eyre::Result<LibmdbxTx<RO>> {
        let tx = LibmdbxTx::new_ro_tx(&self.0)?;

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use brontes_database::clickhouse::Clickhouse;
    use reth_db::cursor::DbCursorRO;
    use serial_test::serial;

    use super::{initialize::LibmdbxInitializer, *};
    use crate::tables::TokenDecimals;

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
