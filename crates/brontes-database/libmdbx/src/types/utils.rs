pub(crate) mod pool_tokens {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::types::address_to_tokens::PoolTokens;

    pub fn serialize<S: Serializer>(u: &PoolTokens, serializer: S) -> Result<S::Ok, S::Error> {
        u.clone()
            .into_iter()
            .map(|a| format!("{:?}", a))
            .collect::<Vec<String>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolTokens, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: (Vec<String>, u64) = Deserialize::deserialize(deserializer)?;

        Ok(addresses.into())
    }
}

pub(crate) mod static_bindings {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::types::address_to_protocol::StaticBindingsDb;

    pub fn serialize<S: Serializer>(
        u: &StaticBindingsDb,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = (*u).into();
        st.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<StaticBindingsDb, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(address.unwrap().into())
    }
}

pub(crate) mod u256 {

    use std::str::FromStr;

    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        U256::from_str(&data).map_err(serde::de::Error::custom)
    }
}

pub(crate) mod address {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(u: &Address, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Address::from_str(&data).map_err(serde::de::Error::custom)
    }
}

pub(crate) mod vec_txhash {

    use std::str::FromStr;

    use alloy_primitives::TxHash;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(u: &Vec<TxHash>, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<TxHash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        data.into_iter()
            .map(|d| TxHash::from_str(&d))
            .collect::<Result<Vec<_>, <TxHash as FromStr>::Err>>()
            .map_err(serde::de::Error::custom)
    }
}

pub(crate) mod option_address {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Option<Address>, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: Option<String> = Deserialize::deserialize(deserializer)?;
        let data = des.map(|d| Address::from_str(&d));

        if let Some(d) = data {
            Ok(Some(d.map_err(serde::de::Error::custom)?))
        } else {
            Ok(None)
        }
    }
}

pub(crate) mod pool_key {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use brontes_pricing::types::PoolKey;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(u: &PoolKey, serializer: S) -> Result<S::Ok, S::Error> {
        let val = (format!("{:?}", u.pool), u.run, u.batch, u.update_nonce);
        val.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (pool, run, batch, update_nonce): (String, u64, u64, u16) =
            Deserialize::deserialize(deserializer)?;

        Ok(PoolKey {
            pool: Address::from_str(&pool).map_err(serde::de::Error::custom)?,
            run,
            batch,
            update_nonce,
        })
    }
}

pub(crate) mod pool_state {

    use brontes_pricing::types::PoolStateSnapShot;
    use serde::{
        de::{Deserialize, Deserializer, Error as DesError},
        ser::{Error as SerError, Serialize, Serializer},
    };

    use crate::types::pool_state::PoolStateType;

    pub fn serialize<S: Serializer>(
        u: &PoolStateSnapShot,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
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

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolStateSnapShot, D::Error>
    where
        D: Deserializer<'de>,
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

pub mod pools_libmdbx {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::types::pool_creation_block::PoolsLibmdbx;

    pub fn serialize<S: Serializer>(u: &PoolsLibmdbx, serializer: S) -> Result<S::Ok, S::Error> {
        let st: Vec<String> =
            u.0.clone()
                .into_iter()
                .map(|addr| format!("{:?}", addr.clone()))
                .collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolsLibmdbx, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(PoolsLibmdbx(
            data.into_iter()
                .map(|d| Address::from_str(&d))
                .collect::<Result<Vec<_>, <Address as FromStr>::Err>>()
                .map_err(serde::de::Error::custom)?,
        ))
    }
}
