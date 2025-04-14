use brontes_libmdbx::{TransactionKind, DBI, RO, RW};
use reth_db::{
    table::DupSort,
    transaction::{DbTx, DbTxMut},
    DatabaseError,
};

use super::cursor::CompressedCursor;
use crate::libmdbx::{implementation::native::tx::LibmdbxTx, types::CompressedTable, DatabaseEnv};
pub struct CompressedLibmdbxTx<K: TransactionKind>(pub(crate) LibmdbxTx<K>);

impl<K: TransactionKind> CompressedLibmdbxTx<K> {
    /// Gets a table database handle if it exists, otherwise creates it.
    pub fn get_dbi<T>(&self) -> Result<DBI, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.get_dbi::<T>()
    }

    /// Create db Cursor
    pub fn new_cursor<T>(&self) -> Result<CompressedCursor<T, K>, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(CompressedCursor::new(self.0.new_cursor()?))
    }

    pub fn get<T>(&self, key: T::Key) -> Result<Option<T::DecompressedValue>, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.get::<T>(key).map(|opt| opt.map(Into::into))
    }

    pub fn commit(self) -> Result<bool, DatabaseError> {
        self.0.commit()
    }

    pub fn abort(self) {
        self.0.abort()
    }

    pub fn cursor_read<T>(&self) -> Result<CompressedCursor<T, K>, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(CompressedCursor::new(self.0.new_cursor()?))
    }

    pub fn cursor_dup_read<T>(&self) -> Result<CompressedCursor<T, K>, DatabaseError>
    where
        T: DupSort + CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(CompressedCursor::new(self.0.new_cursor()?))
    }

    pub fn entries<T>(&self) -> Result<usize, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.entries::<T>()
    }
}

impl CompressedLibmdbxTx<RO> {
    pub fn new_ro_tx(env: &DatabaseEnv) -> eyre::Result<Self, DatabaseError> {
        Ok(Self(LibmdbxTx::new_ro_tx(env)?))
    }
}

impl CompressedLibmdbxTx<RW> {
    pub fn new_rw_tx(env: &DatabaseEnv) -> Result<Self, DatabaseError> {
        Ok(Self(LibmdbxTx::new_rw_tx(env)?))
    }

    pub fn append_bytes<T: CompressedTable>(
        &self,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), DatabaseError>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.append_bytes::<T>(key, value)
    }

    pub fn put_bytes<T: CompressedTable>(
        &self,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), DatabaseError>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.put_bytes::<T>(key, value)
    }

    pub fn put<T>(&self, key: T::Key, value: T::DecompressedValue) -> Result<(), DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.put::<T>(key, value.into())
    }

    pub fn delete<T>(
        &self,
        key: T::Key,
        value: Option<T::DecompressedValue>,
    ) -> Result<bool, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.delete::<T>(key, value.map(Into::into))
    }

    pub fn clear<T>(&self) -> Result<(), DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        self.0.clear::<T>()
    }

    pub fn cursor_write<T>(&self) -> Result<CompressedCursor<T, RW>, DatabaseError>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(CompressedCursor::new(self.0.new_cursor()?))
    }

    pub fn cursor_dup_write<T>(&self) -> Result<CompressedCursor<T, RW>, DatabaseError>
    where
        T: DupSort + CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        Ok(CompressedCursor::new(self.0.new_cursor()?))
    }
}
