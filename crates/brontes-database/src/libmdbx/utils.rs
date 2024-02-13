pub mod protocol_info {

    use brontes_types::db::address_to_protocol_info::ProtocolInfo;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &ProtocolInfo, serializer: S) -> Result<S::Ok, S::Error> {
        let entry = (
            u.clone()
                .into_iter()
                .map(|addr| format!("{:?}", addr))
                .collect::<Vec<_>>(),
            u.init_block,
            u.protocol.to_string(),
            u.curve_lp_token,
        );
        entry.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ProtocolInfo, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: (Vec<String>, u64, String, Option<String>) =
            Deserialize::deserialize(deserializer)?;
        Ok(data.into())
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
