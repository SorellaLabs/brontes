use std::{
    fmt,
    ops::{Bound, RangeBounds},
};

use brontes_libmdbx::{TransactionKind, RW};
use reth_db::{
    cursor::{
        DbCursorRO, DbCursorRW, DbDupCursorRO, DbDupCursorRW, DupWalker, RangeWalker,
        ReverseWalker, Walker,
    },
    table::{DupSort, Table},
    DatabaseError,
};

use super::utils::{
    CompressedPairResult, CompressedTableRow, DecompressedValueOnlyResult, IterCompressedPairResult,
};
use crate::libmdbx::{implementation::native::cursor::LibmdbxCursor, types::CompressedTable};
#[derive(Debug)]
pub struct CompressedCursor<T, K>(LibmdbxCursor<T, K>)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    K: TransactionKind;

impl<T, K> CompressedCursor<T, K>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    K: TransactionKind,
{
    pub fn new(inner: LibmdbxCursor<T, K>) -> Self {
        Self(inner)
    }
}

impl<T, K> CompressedCursor<T, K>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    K: TransactionKind,
{
    pub fn first(&mut self) -> CompressedPairResult<T> {
        self.0.first().map(|opt| opt.map(Into::into))
    }

    pub fn seek_exact(&mut self, key: <T as Table>::Key) -> CompressedPairResult<T> {
        self.0.seek_exact(key).map(|opt| opt.map(Into::into))
    }

    pub fn seek(&mut self, key: <T as Table>::Key) -> CompressedPairResult<T> {
        self.0.seek(key).map(|opt| opt.map(Into::into))
    }

    pub fn seek_raw(&mut self, key: &[u8]) -> CompressedPairResult<T> {
        self.0.seek_raw(key).map(|opt| opt.map(Into::into))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> CompressedPairResult<T> {
        self.0.next().map(|opt| opt.map(Into::into))
    }

    pub fn prev(&mut self) -> CompressedPairResult<T> {
        self.0.prev().map(|opt| opt.map(Into::into))
    }

    pub fn last(&mut self) -> CompressedPairResult<T> {
        self.0.last().map(|opt| opt.map(Into::into))
    }

    pub fn current(&mut self) -> CompressedPairResult<T> {
        self.0.current().map(|opt| opt.map(Into::into))
    }

    pub fn walk(
        &mut self,
        start_key: Option<T::Key>,
    ) -> Result<CompressedWalker<'_, T, LibmdbxCursor<T, K>>, DatabaseError> {
        self.0
            .walk(start_key)
            .map(|walker| CompressedWalker(walker))
    }

    pub fn walk_range(
        &mut self,
        range: impl RangeBounds<T::Key>,
    ) -> Result<CompressedRangeWalker<'_, T, LibmdbxCursor<T, K>>, DatabaseError> {
        self.0
            .walk_range(range)
            .map(|walker| CompressedRangeWalker(walker))
    }

    pub fn walk_back(
        &mut self,
        start_key: Option<T::Key>,
    ) -> Result<CompressedReverseWalker<'_, T, LibmdbxCursor<T, K>>, DatabaseError> {
        self.0
            .walk_back(start_key)
            .map(|walker| CompressedReverseWalker(walker))
    }
}

impl<T, K> CompressedCursor<T, K>
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    K: TransactionKind,
{
    pub fn next_dup(&mut self) -> CompressedPairResult<T> {
        self.0.next_dup().map(|opt| opt.map(Into::into))
    }

    pub fn next_no_dup(&mut self) -> CompressedPairResult<T> {
        self.0.next_no_dup().map(|opt| opt.map(Into::into))
    }

    pub fn next_dup_val(&mut self) -> DecompressedValueOnlyResult<T> {
        self.0.next_dup_val().map(|opt| opt.map(Into::into))
    }

    pub fn seek_by_key_subkey(
        &mut self,
        key: <T as Table>::Key,
        subkey: <T as DupSort>::SubKey,
    ) -> DecompressedValueOnlyResult<T> {
        self.0
            .seek_by_key_subkey(key, subkey)
            .map(|opt| opt.map(Into::into))
    }

    pub fn walk_dup(
        &mut self,
        key: Option<T::Key>,
        subkey: Option<T::SubKey>,
    ) -> Result<CompressedDupWalker<'_, T, LibmdbxCursor<T, K>>, DatabaseError> {
        self.0
            .walk_dup(key, subkey)
            .map(|walker| CompressedDupWalker(walker))
    }
}

impl<T> CompressedCursor<T, RW>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    pub fn upsert(
        &mut self,
        key: T::Key,
        value: T::DecompressedValue,
    ) -> Result<(), DatabaseError> {
        self.0.upsert(key, &(value.into()))
    }

    pub fn insert(
        &mut self,
        key: T::Key,
        value: T::DecompressedValue,
    ) -> Result<(), DatabaseError> {
        self.0.insert(key, &(value.into()))
    }

    pub fn append(
        &mut self,
        key: T::Key,
        value: T::DecompressedValue,
    ) -> Result<(), DatabaseError> {
        self.0.append(key, &(value.into()))
    }

    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current()
    }
}

impl<T: DupSort> CompressedCursor<T, RW>
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    pub fn delete_current_duplicates(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current_duplicates()
    }

    pub fn append_dup(
        &mut self,
        key: T::Key,
        value: T::DecompressedValue,
    ) -> Result<(), DatabaseError> {
        self.0.append_dup(key, value.into())
    }
}

pub struct CompressedWalker<'cursor, T, CURSOR>(Walker<'cursor, T, CURSOR>)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>;

impl<T, CURSOR> fmt::Debug for CompressedWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T, CURSOR> Iterator for CompressedWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    type Item = Result<CompressedTableRow<T>, DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|opt| opt.map(Into::into))
    }
}

impl<'cursor, T, CURSOR> CompressedWalker<'cursor, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    pub fn new(cursor: &'cursor mut CURSOR, start: IterCompressedPairResult<T>) -> Self {
        Self(Walker::new(cursor, start.map(|opt| opt.map(Into::into))))
    }

    pub fn rev(self) -> CompressedReverseWalker<'cursor, T, CURSOR> {
        CompressedReverseWalker(self.0.rev())
    }
}

impl<T, CURSOR> CompressedWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRW<T> + DbCursorRO<T>,
{
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current()
    }
}

pub struct CompressedReverseWalker<'cursor, T, CURSOR>(ReverseWalker<'cursor, T, CURSOR>)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>;

impl<T, CURSOR> fmt::Debug for CompressedReverseWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'cursor, T, CURSOR> CompressedReverseWalker<'cursor, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    pub fn new(cursor: &'cursor mut CURSOR, start: IterCompressedPairResult<T>) -> Self {
        Self(ReverseWalker::new(cursor, start.map(|opt| opt.map(Into::into))))
    }

    pub fn forward(self) -> CompressedWalker<'cursor, T, CURSOR> {
        CompressedWalker(self.0.forward())
    }
}

impl<T, CURSOR> CompressedReverseWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRW<T> + DbCursorRO<T>,
{
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current()
    }
}

impl<T, CURSOR> Iterator for CompressedReverseWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    type Item = Result<CompressedTableRow<T>, DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|opt| opt.map(Into::into))
    }
}

pub struct CompressedRangeWalker<'cursor, T, CURSOR>(RangeWalker<'cursor, T, CURSOR>)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>;

impl<T, CURSOR> fmt::Debug for CompressedRangeWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T, CURSOR> Iterator for CompressedRangeWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    type Item = Result<CompressedTableRow<T>, DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|opt| opt.map(Into::into))
    }
}

impl<'cursor, T, CURSOR> CompressedRangeWalker<'cursor, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRO<T>,
{
    pub fn new(
        cursor: &'cursor mut CURSOR,
        start: IterCompressedPairResult<T>,
        end_key: Bound<T::Key>,
    ) -> Self {
        CompressedRangeWalker(RangeWalker::new(
            cursor,
            start.map(|opt| opt.map(Into::into)),
            end_key,
        ))
    }
}

impl<T, CURSOR> CompressedRangeWalker<'_, T, CURSOR>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRW<T> + DbCursorRO<T>,
{
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current()
    }
}

pub struct CompressedDupWalker<'cursor, T, CURSOR>(DupWalker<'cursor, T, CURSOR>)
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbDupCursorRO<T>;

impl<T, CURSOR> fmt::Debug for CompressedDupWalker<'_, T, CURSOR>
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbDupCursorRO<T> + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T, CURSOR> CompressedDupWalker<'_, T, CURSOR>
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbCursorRW<T> + DbDupCursorRO<T>,
{
    pub fn delete_current(&mut self) -> Result<(), DatabaseError> {
        self.0.delete_current()
    }
}

impl<T, CURSOR> Iterator for CompressedDupWalker<'_, T, CURSOR>
where
    T: DupSort + CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    CURSOR: DbDupCursorRO<T>,
{
    type Item = Result<CompressedTableRow<T>, DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|opt| opt.map(Into::into))
    }
}
