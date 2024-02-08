use reth_db::{
    table::{Table, TableRow},
    DatabaseError,
};

use crate::libmdbx::types::CompressedTable;

#[derive(Debug)]
pub struct CompressedTableRow<T>(
    pub <T as Table>::Key,
    pub <T as CompressedTable>::DecompressedValue,
)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>;

impl<T> PartialEq for CompressedTableRow<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T> From<(<T as Table>::Key, <T as Table>::Value)> for CompressedTableRow<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn from(value: TableRow<T>) -> Self {
        CompressedTableRow(value.0, value.1.into())
    }
}

impl<T> From<CompressedTableRow<T>> for TableRow<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn from(val: CompressedTableRow<T>) -> TableRow<T> {
        (val.0, val.1.into())
    }
}

pub type CompressedPairResult<T> = Result<Option<CompressedTableRow<T>>, DatabaseError>;

pub type DecompressedValueOnlyResult<T> =
    Result<Option<<T as CompressedTable>::DecompressedValue>, DatabaseError>;

pub type IterCompressedPairResult<T> = Option<Result<CompressedTableRow<T>, DatabaseError>>;
