pub(crate) mod address_string {
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

pub(crate) mod pool_tokens {
    use std::str::FromStr;

    use alloy_primitives::Address;
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
        let addresses: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(addresses.into())
    }
}

pub(crate) mod static_bindings {

    use crate::types::address_to_protocol::StaticBindingsDb;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };



    pub fn serialize<S: Serializer>(
        u: &StaticBindingsDb,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = u.clone().into();
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
