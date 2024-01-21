pub mod pool_tokens {

    use brontes_types::db::address_to_tokens::PoolTokens;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

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

pub mod static_bindings {

    use std::str::FromStr;

    use brontes_types::exchanges::StaticBindingsDb;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(
        u: &StaticBindingsDb,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = (*u).to_string();
        st.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<StaticBindingsDb, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(StaticBindingsDb::from_str(&address.unwrap()).unwrap())
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
    use brontes_types::db::redefined_types::primitives::Redefined_Address;
    use redefined::RedefinedConvert;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

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

    use brontes_types::db::redefined_types::primitives::Redefined_Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

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
    use brontes_types::db::pool_creation_block::PoolsToAddresses;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

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
