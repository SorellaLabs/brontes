use std::{
    borrow::Cow,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
};

use brontes_libmdbx::{Error, TransactionKind, WriteFlags, RW};
use reth_db::{
    common::{PairResult, ValueOnlyResult},
    cursor::{
        DbCursorRO, DbCursorRW, DbDupCursorRO, DbDupCursorRW, DupWalker, RangeWalker,
        ReverseWalker, Walker,
    },
    table::{DupSort, Encode, Table},
    DatabaseError, DatabaseWriteOperation,
};
use reth_storage_errors::db::DatabaseWriteError;

use super::utils::{decode_one, decode_value, decoder, uncompressable_ref_util};

#[macro_export]
macro_rules! decode {
    ($v:expr) => {
        $v.map_err(|e| reth_db::DatabaseError::Read(e.into()))?
            .map(decoder::<T>)
            .transpose()
    };
}

/// Cursor wrapper to access KV items.
#[derive(Debug)]
pub struct LibmdbxCursor<T: Table, K: TransactionKind> {
    /// Inner `libmdbx` cursor.
    pub(crate) inner: brontes_libmdbx::Cursor<K>,
    /// Phantom data to enforce encoding/decoding.
    _dbi:             PhantomData<T>,
}

impl<T: Table, K: TransactionKind> LibmdbxCursor<T, K> {
    pub(crate) fn new(inner: brontes_libmdbx::Cursor<K>) -> Self {
        Self { inner, _dbi: PhantomData }
    }

    pub fn seek_raw(&mut self, key: &[u8]) -> PairResult<T> {
        decode!(self.inner.set_key(key))
    }
}

/// Takes `(key, value)` from the database and decodes it appropriately.

impl<T: Table, K: TransactionKind> DbCursorRO<T> for LibmdbxCursor<T, K> {
    fn first(&mut self) -> PairResult<T> {
        decode!(self.inner.first())
    }

    fn seek_exact(&mut self, key: <T as Table>::Key) -> PairResult<T> {
        decode!(self.inner.set_key(key.encode().as_ref()))
    }

    fn seek(&mut self, key: <T as Table>::Key) -> PairResult<T> {
        decode!(self.inner.set_range(key.encode().as_ref()))
    }

    fn next(&mut self) -> PairResult<T> {
        decode!(self.inner.next())
    }

    fn prev(&mut self) -> PairResult<T> {
        decode!(self.inner.prev())
    }

    fn last(&mut self) -> PairResult<T> {
        decode!(self.inner.last())
    }

    fn current(&mut self) -> PairResult<T> {
        decode!(self.inner.get_current())
    }

    fn walk(&mut self, start_key: Option<T::Key>) -> Result<Walker<'_, T, Self>, DatabaseError> {
        let start = if let Some(start_key) = start_key {
            self.inner
                .set_range(start_key.encode().as_ref())
                .map_err(|e| DatabaseError::Read(e.into()))?
                .map(decoder::<T>)
        } else {
            self.first().transpose()
        };

        Ok(Walker::new(self, start))
    }

    fn walk_range(
        &mut self,
        range: impl RangeBounds<T::Key>,
    ) -> Result<RangeWalker<'_, T, Self>, DatabaseError> {
        let start = match range.start_bound().cloned() {
            Bound::Included(key) => self.inner.set_range(key.encode().as_ref()),
            Bound::Excluded(_key) => {
                unreachable!("Rust doesn't allow for Bound::Excluded in starting bounds");
            }
            Bound::Unbounded => self.inner.first(),
        }
        .map_err(|e| DatabaseError::Read(e.into()))?
        .map(decoder::<T>);

        Ok(RangeWalker::new(self, start, range.end_bound().cloned()))
    }

    fn walk_back(
        &mut self,
        start_key: Option<T::Key>,
    ) -> Result<ReverseWalker<'_, T, Self>, DatabaseError> {
        let start = if let Some(start_key) = start_key {
            decode!(self.inner.set_range(start_key.encode().as_ref()))
        } else {
            self.last()
        }
        .transpose();

        Ok(ReverseWalker::new(self, start))
    }
}

impl<T: DupSort, K: TransactionKind> DbDupCursorRO<T> for LibmdbxCursor<T, K> {
    /// Returns the next `(key, value)` pair of a DUPSORT table.
    fn next_dup(&mut self) -> PairResult<T> {
        decode!(self.inner.next_dup())
    }

    /// Returns the next `(key, value)` pair skipping the duplicates.
    fn next_no_dup(&mut self) -> PairResult<T> {
        decode!(self.inner.next_nodup())
    }

    /// Returns the next `value` of a duplicate `key`.
    fn next_dup_val(&mut self) -> ValueOnlyResult<T> {
        self.inner
            .next_dup()
            .map_err(|e| DatabaseError::Read(e.into()))?
            .map(decode_value::<T>)
            .transpose()
    }

    fn seek_by_key_subkey(
        &mut self,
        key: <T as Table>::Key,
        subkey: <T as DupSort>::SubKey,
    ) -> ValueOnlyResult<T> {
        self.inner
            .get_both_range(key.encode().as_ref(), subkey.encode().as_ref())
            .map_err(|e| DatabaseError::Read(e.into()))?
            .map(decode_one::<T>)
            .transpose()
    }

    /// Depending on its arguments, returns an iterator starting at:
    /// - Some(key), Some(subkey): a `key` item whose data is >= than `subkey`
    /// - Some(key), None: first item of a specified `key`
    /// - None, Some(subkey): like first case, but in the first key
    /// - None, None: first item in the table of a DUPSORT table.
    fn walk_dup(
        &mut self,
        key: Option<T::Key>,
        subkey: Option<T::SubKey>,
    ) -> Result<DupWalker<'_, T, Self>, DatabaseError> {
        let start = match (key, subkey) {
            (Some(key), Some(subkey)) => {
                // encode key and decode it after.
                let key: Vec<u8> = key.encode().into();
                self.inner
                    .get_both_range(key.as_ref(), subkey.encode().as_ref())
                    .map_err(|e| DatabaseError::Read(e.into()))?
                    .map(|val| decoder::<T>((Cow::Owned(key), val)))
            }
            (Some(key), None) => {
                let key: Vec<u8> = key.encode().into();
                self.inner
                    .set(key.as_ref())
                    .map_err(|e| DatabaseError::Read(e.into()))?
                    .map(|val| decoder::<T>((Cow::Owned(key), val)))
            }
            (None, Some(subkey)) => {
                if let Some((key, _)) = self.first()? {
                    let key: Vec<u8> = key.encode().into();
                    self.inner
                        .get_both_range(key.as_ref(), subkey.encode().as_ref())
                        .map_err(|e| DatabaseError::Read(e.into()))?
                        .map(|val| decoder::<T>((Cow::Owned(key), val)))
                } else {
                    let err_code = Error::to_err_code(&Error::NotFound);
                    Some(Err(DatabaseError::Read(err_code.into())))
                }
            }
            (None, None) => self.first().transpose(),
        };

        Ok(DupWalker::<'_, T, Self> { cursor: self, start })
    }
}

impl<T: Table> DbCursorRW<T> for LibmdbxCursor<T, RW> {
    /// Database operation that will update an existing row if a specified value
    /// already exists in a table, and insert a new row if the specified
    /// value doesn't already exist
    ///
    /// For a DUPSORT table, `upsert` will not actually update-or-insert. If the
    /// key already exists, it will append the value to the subkey, even if
    /// the subkeys are the same. So if you want to properly upsert, you'll
    /// need to `seek_exact` & `delete_current` if the key+subkey was found,
    /// before calling `upsert`.
    fn upsert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let (key, value) = uncompressable_ref_util::<T>(key, value);
        self.inner
            .put(&key, &value, WriteFlags::UPSERT)
            .map_err(|e| {
                DatabaseWriteError {
                    info: e.into(),
                    operation: DatabaseWriteOperation::CursorUpsert,
                    table_name: T::NAME,
                    key,
                }
                .into()
            })
    }

    fn insert(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let (key, value) = uncompressable_ref_util::<T>(key, value);
        self.inner
            .put(&key, &value, WriteFlags::NO_OVERWRITE)
            .map_err(|e| {
                DatabaseWriteError {
                    info: e.into(),
                    operation: DatabaseWriteOperation::CursorInsert,
                    table_name: T::NAME,
                    key,
                }
                .into()
            })
    }

    /// Appends the data to the end of the table. Consequently, the append
    /// operation will fail if the inserted key is less than the last table
    /// key
    fn append(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let (key, value) = uncompressable_ref_util::<T>(key, value);
        self.inner
            .put(&key, &value, WriteFlags::APPEND)
            .map_err(|e| {
                DatabaseWriteError {
                    info: e.into(),
                    operation: DatabaseWriteOperation::CursorAppend,
                    table_name: T::NAME,
                    key,
                }
                .into()
            })
    }

    fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.inner
            .del(WriteFlags::CURRENT)
            .map_err(|e| DatabaseError::Delete(e.into()))
    }
}

impl<T: DupSort> DbDupCursorRW<T> for LibmdbxCursor<T, RW> {
    fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError> {
        self.inner
            .del(WriteFlags::NO_DUP_DATA)
            .map_err(|e| DatabaseError::Delete(e.into()))
    }

    fn append_dup(&mut self, key: T::Key, value: T::Value) -> Result<(), DatabaseError> {
        let (key, value) = uncompressable_ref_util::<T>(key, value);
        self.inner
            .put(&key, &value, WriteFlags::APPEND_DUP)
            .map_err(|e| {
                DatabaseWriteError {
                    info: e.into(),
                    operation: DatabaseWriteOperation::CursorAppendDup,
                    table_name: T::NAME,
                    key,
                }
                .into()
            })
    }
}
