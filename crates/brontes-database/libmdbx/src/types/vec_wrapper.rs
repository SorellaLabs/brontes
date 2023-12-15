use arbitrary::Arbitrary;
use codecs_derive::main_codec;
use reth_codecs::Compact;
use reth_db::table::{Compress, Decompress, Value};
use reth_primitives::bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct VecWrapper<T: Serialize + Compress + Decompress>(Vec<T>);

impl<T: Serialize + Compress + Decompress> IntoIterator for VecWrapper<T> {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Serialize + Compress + Decompress> Serialize for VecWrapper<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: for<'a> Deserialize<'a> + Serialize + Compress + Decompress> Deserialize<'de>
    for VecWrapper<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner: Vec<T> = Deserialize::deserialize(deserializer)?;

        Ok(Self(inner))
    }
}

impl<T: Serialize + Compress + Decompress> From<Vec<T>> for VecWrapper<T> {
    fn from(value: Vec<T>) -> Self {
        Self(value)
    }
}

impl<T: Serialize + Compress + Decompress> Into<Vec<T>> for VecWrapper<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}

impl<T> Decompress for VecWrapper<T>
where
    T: Serialize + Compress + Decompress,
    Vec<T>: Decompress,
{
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        Ok(VecWrapper(Vec::<T>::decompress(value)?))
    }
}

impl<T> Compress for VecWrapper<T>
where
    T: Serialize + Compress + Decompress,
    Vec<T>: Compress,
{
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        self.0.compress_to_buf(buf)
    }
}

/*
impl<'a, T: Arbitrary<'a>> Arbitrary<'a> for VecWrapper<T> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {

    }
}


impl<T> ScaleValue for VecWrapper<T>
where
    T: ScaleValue,
    Vec<T>: ScaleValue,
{
}

impl<T> reth_db::table::Value for VecWrapper<T>
where
    T: Value,
    Vec<T>: Value,
{
}

 */

impl<T> Compact for VecWrapper<T>
where
    T: Serialize + Compress + Decompress,
    Vec<T>: Compact,
{
    #[inline]
    fn to_compact<B>(self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        self.0.to_compact(buf)
    }

    #[inline]
    fn from_compact(buf: &[u8], identifier: usize) -> (Self, &[u8]) {
        let (i, buf) = Vec::<T>::from_compact(buf, identifier);
        (VecWrapper(Vec::from(i)), buf)
    }
}
