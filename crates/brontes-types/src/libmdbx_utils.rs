#[macro_export]
macro_rules! impl_compress_decompress_for_encoded_decoded {
    ($type:ty) => {
        impl reth_db::table::Compress for $type {
            type Compressed = Vec<u8>;

            fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
                let mut encoded = Vec::new();
                self.encode(&mut encoded);
                buf.put_slice(&encoded);
            }
        }

        impl reth_db::table::Decompress for $type {
            fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
                let binding = value.as_ref().to_vec();
                let buf = &mut binding.as_slice();
                Ok(Self::decode(buf).map_err(|_| reth_db::DatabaseError::Decode)?)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_compress_decompress_for_serde {
    ($type:ty) => {
        impl reth_db::table::Compress for $type {
            type Compressed = Vec<u8>;

            fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
                let bytes = serde_json::to_vec(&self).unwrap();
                buf.put_slice(&bytes);
            }
        }

        impl reth_db::table::Decompress for $type {
            fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
                let binding = value.as_ref().to_vec();
                let buf = &binding.as_slice();
                println!("decoding buf, {buf:#?}");
                Ok(serde_json::from_slice(buf).map_err(|_| reth_db::DatabaseError::Decode)?)
            }
        }
    };
}

/*
pub mod serde_hashmap {

    use std::{collections::HashMap, str::FromStr};

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer, Error as DesError},
        ser::{Error as SerError, Serialize, Serializer},
    };

    pub fn serialize<S: Serializer, T, K>(
        u: &HashMap<T, K>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
        K: Serialize,
    {
        let pool_state = match u {
            PoolStateSnapShot::UniswapV2(pool) => {
                serde_json::to_string(pool).map_err(|err| S::Error::custom(err.to_string()))?
            }
            PoolStateSnapShot::UniswapV3(pool) => {
                serde_json::to_string(pool).map_err(|err| S::Error::custom(err.to_string()))?
            }
        };

        pool_state.serialize(serializer)
    }

    pub fn deserialize<'de, D, T, K>(deserializer: D) -> Result<HashMap<T, K>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
        K: Deserialize<'de>,
    {
        let (pool_type, pool_state): (PoolStateType, String) =
            Deserialize::deserialize(deserializer)?;

        let pool = match pool_type {
            PoolStateType::UniswapV2 => PoolStateSnapShot::UniswapV2(
                serde_json::from_str(&pool_state)
                    .map_err(|err| D::Error::custom(err.to_string()))?,
            ),
            PoolStateType::UniswapV3 => PoolStateSnapShot::UniswapV2(
                serde_json::from_str(&pool_state)
                    .map_err(|err| D::Error::custom(err.to_string()))?,
            ),
        };

        Ok(pool)
    }
}
 */
