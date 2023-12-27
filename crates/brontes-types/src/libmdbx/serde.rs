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

        Ok(Address::from_str(&address).map_err(serde::de::Error::custom)?)
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

        Ok(data
            .into_iter()
            .map(|d| Address::from_str(&d))
            .collect::<Result<Vec<_>, <Address as FromStr>::Err>>()
            .map_err(serde::de::Error::custom)?)
    }
}
