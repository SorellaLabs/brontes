#[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
use std::collections::HashMap;

#[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
use serde::de::{DeserializeOwned, Error};
#[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
use serde_json::{Error as SerdeJsonError, Value};

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

pub mod vec_address {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer, T: Into<Address> + Debug>(
        u: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: Vec<String> = u
            .iter()
            .map(|addr| format!("{:?}", addr))
            .collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D, T: From<Address>>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        data.into_iter()
            .map(|d| Address::from_str(&d).map(Into::into))
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

    pub fn serialize<S: Serializer>(u: &[U256], serializer: S) -> Result<S::Ok, S::Error> {
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

    pub fn serialize<S: Serializer>(u: &[Vec<U256>], serializer: S) -> Result<S::Ok, S::Error> {
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

#[allow(dead_code)]
pub(crate) mod vec_vec_fixed_string {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use clickhouse::fixed_string::FixedString;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &[Vec<Address>], serializer: S) -> Result<S::Ok, S::Error> {
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
pub(crate) mod vec_vec_b256 {

    use std::str::FromStr;

    use alloy_primitives::B256;
    use clickhouse::fixed_string::FixedString;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &[Vec<B256>], serializer: S) -> Result<S::Ok, S::Error> {
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

pub mod u256 {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer, T: Into<U256> + Debug>(
        u: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u);
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D, T: From<U256>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut data: String = Deserialize::deserialize(deserializer)?;

        if data.ends_with("_U256") {
            data = data[..data.len() - 5].to_string()
        }
        Ok(U256::from_str(&data)
            .map_err(serde::de::Error::custom)?
            .into())
    }
}

pub mod vec_b256 {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::B256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer, T: Into<B256> + Debug>(
        u: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: Vec<String> = u.iter().map(|data| format!("{:?}", data)).collect();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D, T: From<B256>>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        data.into_iter()
            .map(|d| B256::from_str(&d).map(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
    }
}

pub mod vec_bls_pub_key {

    use std::{fmt::Debug, str::FromStr};

    use reth_rpc_types::beacon::BlsPublicKey;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer, T: Into<BlsPublicKey> + Debug>(
        u: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: Vec<String> = u.iter().map(|data| format!("{:?}", data)).collect();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D, T: From<BlsPublicKey>>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        data.into_iter()
            .map(|d| BlsPublicKey::from_str(&d).map(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)
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

pub mod static_bindings {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::protocol::Protocol;

    pub fn serialize<S: Serializer>(u: &Protocol, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = (*u).to_string();
        st.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Protocol, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(Protocol::from_db_string(&address.unwrap()))
    }
}

pub mod addresss {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer, T: Into<Address> + Debug>(
        u: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u);
        st.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D, T: From<Address>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Address::from_str(&data)
            .map_err(serde::de::Error::custom)
            .map(Into::into)
    }
}

pub mod option_addresss {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer, T: Into<Address> + Debug>(
        u: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: Option<String> = u.as_ref().map(|inner| format!("{:?}", inner));
        st.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D, T: From<Address>>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data_option: Option<String> = Deserialize::deserialize(deserializer)?;

        data_option
            .map(|data| {
                Address::from_str(&data)
                    .map_err(serde::de::Error::custom)
                    .map(Into::into)
            })
            .transpose()
    }
}

pub mod vec_txhash {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::TxHash;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer, D: Into<TxHash> + Debug>(
        u: &[D],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let data = u.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>();

        data.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D, T: From<TxHash>>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(data
            .into_iter()
            .map(|d| TxHash::from_str(&d))
            .collect::<Result<Vec<_>, <TxHash as FromStr>::Err>>()
            .map_err(serde::de::Error::custom)?
            .into_iter()
            .map(|t| t.into())
            .collect())
    }
}

pub mod txhash {

    use std::{fmt::Debug, str::FromStr};

    use alloy_primitives::TxHash;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer, D: Into<TxHash> + Debug>(
        u: &D,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let data = format!("{:?}", u);

        data.serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D, T: From<TxHash>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Ok(TxHash::from_str(&data).unwrap().into())
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

    use crate::db::redefined_types::primitives::AddressRedefined;

    pub fn serialize<S: Serializer>(
        u: &Option<AddressRedefined>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<AddressRedefined>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: Option<String> = Deserialize::deserialize(deserializer)?;
        let data = des.map(|d| Address::from_str(&d));

        if let Some(d) = data {
            Ok(Some(AddressRedefined::from_source(d.map_err(serde::de::Error::custom)?)))
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

    use crate::db::redefined_types::primitives::AddressRedefined;

    pub fn serialize<S: Serializer>(
        u: &AddressRedefined,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AddressRedefined, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: String = Deserialize::deserialize(deserializer)?;
        AddressRedefined::from_str(&des).map_err(serde::de::Error::custom)
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

pub mod option_contract_info {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::Deserialize,
        ser::{Serialize, Serializer},
        Deserializer,
    };

    use crate::db::address_metadata::ContractInfo;

    #[cfg(any(not(feature = "api-des"), feature = "api", feature = "local-clickhouse"))]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<ContractInfo>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (verified_contract, contract_creator_opt, reputation): (
            Option<bool>,
            Option<String>,
            Option<u8>,
        ) = Deserialize::deserialize(deserializer)?;

        Ok(contract_creator_opt.map(|contract_creator| ContractInfo {
            verified_contract,
            contract_creator: Address::from_str(&contract_creator).ok(),
            reputation,
        }))
    }

    #[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<ContractInfo>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::collections::HashMap;

        use serde::de::Error;

        use super::get_val_from_map;

        let Some(contract_info_map): Option<HashMap<String, serde_json::Value>> =
            Deserialize::deserialize(deserializer)?
        else {
            return Ok(None)
        };

        let verified_contract =
            get_val_from_map(&contract_info_map, "verified_contract").map_err(D::Error::custom)?;

        let contract_creator_opt: Option<String> =
            get_val_from_map(&contract_info_map, "contract_creator").map_err(D::Error::custom)?;

        let reputation =
            get_val_from_map(&contract_info_map, "reputation").map_err(D::Error::custom)?;

        let contract_info = contract_creator_opt.map(|contract_creator| ContractInfo {
            verified_contract,
            contract_creator: Address::from_str(&contract_creator).ok(),
            reputation,
        });

        Ok(contract_info)
    }
    pub fn serialize<S: Serializer>(u: &ContractInfo, serializer: S) -> Result<S::Ok, S::Error> {
        (u.verified_contract, u.contract_creator, u.reputation).serialize(serializer)
    }
}

pub mod socials {

    use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

    use crate::db::address_metadata::Socials;
    #[cfg(any(not(feature = "api-des"), feature = "api", feature = "local-clickhouse"))]
    type SocalDecode =
        (Option<String>, Option<u64>, Option<String>, Option<String>, Option<String>);

    #[cfg(any(not(feature = "api-des"), feature = "api", feature = "local-clickhouse"))]
    pub fn deserialize<'de, D, T: From<Socials>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (twitter, twitter_followers, website_url, crunchbase, linkedin): SocalDecode =
            Deserialize::deserialize(deserializer)?;

        Ok(Socials { twitter, twitter_followers, website_url, crunchbase, linkedin }.into())
    }

    #[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
    pub fn deserialize<'de, D, T: From<Socials>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::collections::HashMap;

        use serde::de::Error;

        use super::get_val_from_map;

        let socials_map: HashMap<String, serde_json::Value> =
            Deserialize::deserialize(deserializer)?;

        Ok(Socials {
            twitter:           get_val_from_map(&socials_map, "twitter")
                .map_err(D::Error::custom)?,
            twitter_followers: get_val_from_map(&socials_map, "twitter_followers")
                .map_err(D::Error::custom)?,
            website_url:       get_val_from_map(&socials_map, "website_url")
                .map_err(D::Error::custom)?,
            crunchbase:        get_val_from_map(&socials_map, "crunchbase")
                .map_err(D::Error::custom)?,
            linkedin:          get_val_from_map(&socials_map, "linkedin")
                .map_err(D::Error::custom)?,
        }
        .into())
    }

    pub fn serialize<S: Serializer>(u: &Socials, serializer: S) -> Result<S::Ok, S::Error> {
        (&u.twitter, &u.twitter_followers, &u.website_url, &u.crunchbase, &u.linkedin)
            .serialize(serializer)
    }
}

pub mod option_fund {
    use serde::de::{Deserialize, Deserializer};

    use crate::db::searcher::Fund;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Fund>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fund: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(fund.map(Into::into))
    }
}

#[cfg(all(feature = "api-des", not(feature = "api"), not(feature = "local-clickhouse")))]
fn get_val_from_map<T>(map: &HashMap<String, Value>, key: &str) -> Result<T, SerdeJsonError>
where
    T: DeserializeOwned,
{
    let value = map
        .get(key)
        .ok_or_else(|| SerdeJsonError::custom(format!("Could not find value for key {key}")))
        .and_then(|v| serde_json::from_value(v.clone()).map_err(SerdeJsonError::custom))?;

    Ok(value)
}
