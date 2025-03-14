use std::borrow::Cow;

use reth_db::{
    table::{Compress, Decode, Decompress, Encode, Table, TableRow},
    DatabaseError,
};

/// Helper function to decode a `(key, value)` pair.
pub(crate) fn decoder<'a, T>(
    kv: (Cow<'a, [u8]>, Cow<'a, [u8]>),
) -> Result<TableRow<T>, DatabaseError>
where
    T: Table,
    T::Key: Decode,
    T::Value: Decompress,
{
    let key = match kv.0 {
        Cow::Borrowed(k) => Decode::decode(k)?,
        Cow::Owned(k) => Decode::decode(&k)?,
    };
    let value = match kv.1 {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress_owned(v)?,
    };
    Ok((key, value))
}

/// Helper function to decode only a value from a `(key, value)` pair.
pub(crate) fn decode_value<'a, T>(
    kv: (Cow<'a, [u8]>, Cow<'a, [u8]>),
) -> Result<T::Value, DatabaseError>
where
    T: Table,
{
    Ok(match kv.1 {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress_owned(v)?,
    })
}

/// Helper function to decode a value. It can be a key or subkey.
pub(crate) fn decode_one<T>(value: Cow<'_, [u8]>) -> Result<T::Value, DatabaseError>
where
    T: Table,
{
    Ok(match value {
        Cow::Borrowed(v) => Decompress::decompress(v)?,
        Cow::Owned(v) => Decompress::decompress_owned(v)?,
    })
}

pub(crate) fn uncompressable_ref_util<T: Table>(
    key: T::Key,
    value: T::Value,
) -> (Vec<u8>, Vec<u8>) {
    if let Some(val) = value.uncompressable_ref() {
        (key.encode().into(), val.to_vec())
    } else {
        let mut buf = Vec::new();
        value.compress_to_buf(&mut buf);
        (key.encode().into(), buf.to_vec())
    }
}
