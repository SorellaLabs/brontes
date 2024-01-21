pub mod address_string {
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

        Address::from_str(&address).map_err(serde::de::Error::custom)
    }
}

pub mod vec_address_string {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<Address>, serializer: S) -> Result<S::Ok, S::Error> {
        let st: Vec<String> = u
            .iter()
            .map(|addr| format!("{:?}", addr.clone()))
            .collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        data.into_iter()
            .map(|d| Address::from_str(&d))
            .collect::<Result<Vec<_>, <Address as FromStr>::Err>>()
            .map_err(serde::de::Error::custom)
    }
}

pub mod vec_u256 {
    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<U256>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.to_le_bytes())
            .collect::<Vec<[u8; 32]>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<[u8; 32]> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter().map(U256::from_le_bytes).collect())
    }
}
#[allow(dead_code)]
pub(crate) mod vec_vec_u256 {

    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<Vec<U256>>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|u| u.iter().map(|u| u.to_le_bytes()).collect::<Vec<_>>())
            .collect::<Vec<Vec<[u8; 32]>>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<U256>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let u: Vec<Vec<[u8; 32]>> = Deserialize::deserialize(deserializer)?;
        Ok(u.into_iter()
            .map(|i| i.into_iter().map(U256::from_le_bytes).collect())
            .collect())
    }
}

pub(crate) mod vec_fixed_string {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::clickhouse::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<Address>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|a| format!("{:?}", a).into())
            .collect::<Vec<FixedString>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<FixedString> = Deserialize::deserialize(deserializer)?;

        addresses
            .into_iter()
            .map(|a| Address::from_str(&a.string))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
    }
}
#[allow(dead_code)]
pub(crate) mod vec_vec_fixed_string {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::clickhouse::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(
        u: &Vec<Vec<Address>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|addrs| {
                addrs
                    .iter()
                    .map(|a| format!("{:?}", a).into())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<FixedString>>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<Address>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<Vec<FixedString>> = Deserialize::deserialize(deserializer)?;

        addresses
            .into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| Address::from_str(&a.string))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
    }
}
#[allow(dead_code)]
pub(crate) mod vec_b256 {
    use std::str::FromStr;

    use alloy_primitives::B256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::clickhouse::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<B256>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|a| format!("{:?}", a).into())
            .collect::<Vec<FixedString>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<B256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<FixedString> = Deserialize::deserialize(deserializer)?;

        addresses
            .into_iter()
            .map(|a| B256::from_str(&a.string))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
    }
}

#[allow(dead_code)]
pub(crate) mod vec_vec_b256 {

    use std::str::FromStr;

    use alloy_primitives::B256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    use sorella_db_databases::clickhouse::fixed_string::FixedString;

    pub fn serialize<S: Serializer>(u: &Vec<Vec<B256>>, serializer: S) -> Result<S::Ok, S::Error> {
        u.iter()
            .map(|addrs| {
                addrs
                    .iter()
                    .map(|a| format!("{:?}", a).into())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<FixedString>>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<B256>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<Vec<FixedString>> = Deserialize::deserialize(deserializer)?;

        addresses
            .into_iter()
            .map(|addrs| {
                addrs
                    .into_iter()
                    .map(|a| B256::from_str(&a.string))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
    }
}

pub mod pool_tokens {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::address_to_tokens::PoolTokens;

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

pub mod u256 {

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

        Ok(U256::from_str(&data).map_err(serde::de::Error::custom)?)
    }
}

pub mod address {

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

pub mod vec_txhash {

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

pub mod option_r_address {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use redefined::RedefinedConvert;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::redefined_types::primitives::Redefined_Address;

    pub fn serialize<S: Serializer>(
        u: &Option<Redefined_Address>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Redefined_Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: Option<String> = Deserialize::deserialize(deserializer)?;
        let data = des.map(|d| Address::from_str(&d));

        if let Some(d) = data {
            Ok(Some(Redefined_Address::from_source(d.map_err(serde::de::Error::custom)?)))
        } else {
            Ok(None)
        }
    }
}

pub mod option_address {

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

pub mod r_address {

    use std::str::FromStr;

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::redefined_types::primitives::Redefined_Address;

    pub fn serialize<S: Serializer>(
        u: &Redefined_Address,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Redefined_Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: String = Deserialize::deserialize(deserializer)?;
        Redefined_Address::from_str(&des).map_err(serde::de::Error::custom)
    }
}

pub mod pools_libmdbx {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::pool_creation_block::PoolsToAddresses;

    pub fn serialize<S: Serializer>(
        u: &PoolsToAddresses,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: Vec<String> =
            u.0.clone()
                .into_iter()
                .map(|addr| format!("{:?}", addr.clone()))
                .collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolsToAddresses, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(PoolsToAddresses(
            data.into_iter()
                .map(|d| Address::from_str(&d))
                .collect::<Result<Vec<_>, <Address as FromStr>::Err>>()
                .map_err(serde::de::Error::custom)?,
        ))
    }
}
