use sqlx::postgres::{PgHasArrayType, PgTypeInfo};
use sqlx::{Decode, Encode, Postgres, Type};
use alloy_primitives::B256;

pub struct Hash256(pub B256);

impl Type<Postgres> for Hash256 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("hash256")
    }
}

impl Encode<'_, Postgres> for Hash256 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> sqlx::encode::IsNull {
        Encode::<Postgres>::encode(&format!("{}", self.0), buf)
    }
}

impl Decode<'_, Postgres> for Hash256 {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let hash = Decode::<Postgres>::decode(value)?;
        let hash = hash.parse::<B256>()?;
        Ok(Hash256(hash))
    }
}

impl PgHasArrayType for Hash256 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("_hash256")
    }
}