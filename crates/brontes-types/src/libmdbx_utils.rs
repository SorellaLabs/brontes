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

pub mod serde_address_string {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Address, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{:?}", u).serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: String = Deserialize::deserialize(deserializer)?;

        Ok(Address::from_str(&address).map_err(serde::de::Error::custom)?)
    }
}
