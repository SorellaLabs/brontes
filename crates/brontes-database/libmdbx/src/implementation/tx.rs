use std::{str::FromStr, sync::Arc};

use parking_lot::RwLock;
use reth_db::{
    table::{Compress, DupSort, Encode, Table},
    transaction::{DbTx, DbTxMut},
    DatabaseEnv, DatabaseError, DatabaseWriteOperation,
};
use reth_interfaces::db::DatabaseWriteError;
use reth_libmdbx::{ffi::DBI, Transaction, TransactionKind, WriteFlags, RO, RW};

use super::{cursor::LibmdbxCursor, utils::decode_one};
use crate::tables::{Tables, NUM_TABLES};

pub struct LibmdbxTx<K: TransactionKind> {
    /// Libmdbx-sys transaction.
    pub inner:             Transaction<K>,
    /// Database table handle cache.
    pub(crate) db_handles: Arc<RwLock<[Option<DBI>; NUM_TABLES]>>,
}

impl LibmdbxTx<RO> {
    pub(crate) fn new_ro_tx(env: &DatabaseEnv) -> eyre::Result<LibmdbxTx<RO>, DatabaseError> {
        Ok(Self {
            inner:      env
                .begin_ro_txn()
                .map_err(|e| DatabaseError::InitTx(e.into()))?,
            db_handles: Default::default(),
        })
    }
}

impl LibmdbxTx<RW> {
    pub(crate) fn new_rw_tx(env: &DatabaseEnv) -> Result<LibmdbxTx<RW>, DatabaseError> {
        Ok(Self {
            inner:      env
                .begin_rw_txn()
                .map_err(|e| DatabaseError::InitTx(e.into()))?,
            db_handles: Default::default(),
        })
    }
}

impl<K: TransactionKind> LibmdbxTx<K> {
    /// Gets a table database handle if it exists, otherwise creates it.
    pub fn get_dbi<T: Table>(&self) -> Result<DBI, DatabaseError> {
        let mut handles = self.db_handles.write();

        let table = Tables::from_str(T::NAME).expect("Requested table should be part of `Tables`.");

        let dbi_handle = handles.get_mut(table as usize).expect("should exist");
        if dbi_handle.is_none() {
            *dbi_handle = Some(
                self.inner
                    .open_db(Some(T::NAME))
                    .map_err(|e| DatabaseError::InitCursor(e.into()))?
                    .dbi(),
            );
        }

        Ok(dbi_handle.expect("is some; qed"))
    }

    /// Create db Cursor
    pub fn new_cursor<T: Table>(&self) -> Result<LibmdbxCursor<T, K>, DatabaseError> {
        let inner = self
            .inner
            .cursor_with_dbi(self.get_dbi::<T>()?)
            .map_err(|e| DatabaseError::InitCursor(e.into()))?;

        Ok(LibmdbxCursor::new(inner))
    }
}

impl<K: TransactionKind> DbTx for LibmdbxTx<K> {
    type Cursor<T: Table> = LibmdbxCursor<T, K>;
    type DupCursor<T: DupSort> = LibmdbxCursor<T, K>;

    fn get<T: Table>(&self, key: T::Key) -> Result<Option<<T as Table>::Value>, DatabaseError> {
        self.inner
            .get(self.get_dbi::<T>()?, key.encode().as_ref())
            .map_err(|e| DatabaseError::Read(e.into()))?
            .map(decode_one::<T>)
            .transpose()
    }

    fn commit(self) -> Result<bool, DatabaseError> {
        self.inner
            .commit()
            .map(|(res, _latency)| res)
            .map_err(|e| DatabaseError::Commit(e.into()))
    }

    fn abort(self) {
        drop(self.inner)
    }

    // Iterate over read only values in database.
    fn cursor_read<T: Table>(&self) -> Result<Self::Cursor<T>, DatabaseError> {
        self.new_cursor::<T>()
    }

    /// Iterate over read only values in database.
    fn cursor_dup_read<T: DupSort>(&self) -> Result<Self::DupCursor<T>, DatabaseError> {
        self.new_cursor::<T>()
    }

    /// Returns number of entries in the table using cheap DB stats invocation.
    fn entries<T: Table>(&self) -> Result<usize, DatabaseError> {
        Ok(self
            .inner
            .db_stat_with_dbi(self.get_dbi::<T>()?)
            .map_err(|e| DatabaseError::Stats(e.into()))?
            .entries())
    }
}

impl DbTxMut for LibmdbxTx<RW> {
    type CursorMut<T: Table> = LibmdbxCursor<T, RW>;
    type DupCursorMut<T: DupSort> = LibmdbxCursor<T, RW>;

    fn put<T: Table>(&self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let key = key.encode();
        let value = value.compress();
        println!("value compress");
        self.inner
            .put(self.get_dbi::<T>()?, key.as_ref(), value, WriteFlags::UPSERT)
            .map_err(|e| {
                DatabaseWriteError {
                    code:       e.into(),
                    operation:  DatabaseWriteOperation::Put,
                    table_name: T::NAME,
                    key:        key.into(),
                }
                .into()
            })
    }

    fn delete<T: Table>(
        &self,
        key: T::Key,
        value: Option<T::Value>,
    ) -> Result<bool, DatabaseError> {
        let mut data = None;

        let value = value.map(Compress::compress);
        if let Some(value) = &value {
            data = Some(value.as_ref());
        };

        self.inner
            .del(self.get_dbi::<T>()?, key.encode(), data)
            .map_err(|e| DatabaseError::Delete(e.into()))
    }

    fn clear<T: Table>(&self) -> Result<(), DatabaseError> {
        self.inner
            .clear_db(self.get_dbi::<T>()?)
            .map_err(|e| DatabaseError::Delete(e.into()))?;

        Ok(())
    }

    fn cursor_write<T: Table>(&self) -> Result<Self::CursorMut<T>, DatabaseError> {
        self.new_cursor::<T>()
    }

    fn cursor_dup_write<T: DupSort>(&self) -> Result<Self::DupCursorMut<T>, DatabaseError> {
        self.new_cursor::<T>()
    }
}
