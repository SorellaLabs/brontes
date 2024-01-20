use reth_db::{
    table::{Table, TableRow},
    DatabaseError,
};

use crate::CompressedTable;

pub struct CompressedTableRow<T>(
    pub <T as Table>::Key,
    pub <T as CompressedTable>::DecompressedValue,
)
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>;

impl<T> From<(<T as Table>::Key, <T as Table>::Value)> for CompressedTableRow<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn from(value: TableRow<T>) -> Self {
        CompressedTableRow(value.0, value.1.into())
    }
}

impl<T> Into<TableRow<T>> for CompressedTableRow<T>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    fn into(self) -> TableRow<T> {
        (self.0, self.1.into())
    }
}

pub type CompressedPairResult<T> = Result<Option<CompressedTableRow<T>>, DatabaseError>;

pub type DecompressedValueOnlyResult<T> =
    Result<Option<<T as CompressedTable>::DecompressedValue>, DatabaseError>;

pub type IterCompressedPairResult<T> = Option<Result<CompressedTableRow<T>, DatabaseError>>;
