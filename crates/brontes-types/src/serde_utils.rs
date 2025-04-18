pub mod dex_key {
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::dex::{decompose_key, make_key, DexKey};

    pub fn serialize<S: Serializer>(u: &DexKey, serializer: S) -> Result<S::Ok, S::Error> {
        decompose_key(*u).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DexKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (block, tx_idx): (u64, u64) = Deserialize::deserialize(deserializer)?;
        Ok(make_key(block, tx_idx as u16))
    }
}

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

pub mod protocol {

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
pub mod option_txhash {

    use std::str::FromStr;

    use alloy_primitives::TxHash;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };
    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(u: &Option<TxHash>, serializer: S) -> Result<S::Ok, S::Error> {
        u.map(|t| format!("{:?}", t)).serialize(serializer)
    }
    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<TxHash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(data.map(|data| TxHash::from_str(&data).unwrap()))
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
        let st = u.as_ref().map(|u| format!("{:?}", u));
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

    pub fn serialize<S>(value: &Option<ContractInfo>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(contract_info) => (
                contract_info.verified_contract,
                contract_info.contract_creator,
                contract_info.reputation,
            )
                .serialize(serializer),
            None => serializer.serialize_none(),
        }
    }
}

pub mod socials {

    use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

    use crate::db::address_metadata::Socials;
    type SocalDecode =
        (Option<String>, Option<u64>, Option<String>, Option<String>, Option<String>);

    pub fn deserialize<'de, D, T: From<Socials>>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (twitter, twitter_followers, website_url, crunchbase, linkedin): SocalDecode =
            Deserialize::deserialize(deserializer)?;

        Ok(Socials { twitter, twitter_followers, website_url, crunchbase, linkedin }.into())
    }

    pub fn serialize<S: Serializer>(u: &Socials, serializer: S) -> Result<S::Ok, S::Error> {
        (&u.twitter, &u.twitter_followers, &u.website_url, &u.crunchbase, &u.linkedin)
            .serialize(serializer)
    }
}

pub mod option_fund {
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::searcher::Fund;
    pub fn serialize<S: Serializer>(u: &Option<Fund>, serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.map(|f| f.to_string());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Fund>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fund: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(fund.map(Into::into))
    }
}

pub mod vec_fund {
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::searcher::Fund;
    pub fn serialize<S: Serializer>(u: &[Fund], serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.iter().map(|f| f.to_string()).collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Fund>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fund: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(fund.into_iter().map(Into::into).collect::<Vec<_>>())
    }
}

pub mod address_pair {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::pair::Pair;

    pub fn serialize<S: Serializer>(u: &Pair, serializer: S) -> Result<S::Ok, S::Error> {
        let st = (format!("{:?}", u.0), format!("{:?}", u.1));
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Pair, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (data0, data1): (String, String) = Deserialize::deserialize(deserializer)?;

        Ok(Pair(Address::from_str(&data0).unwrap(), Address::from_str(&data1).unwrap()))
    }
}

pub mod option_pair {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::pair::Pair;

    pub fn serialize<S: Serializer>(u: &Option<Pair>, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(u) = u {
            let st = (Some(format!("{:?}", u.0)), Some(format!("{:?}", u.1)));
            st.serialize(serializer)
        } else {
            (None::<String>, None::<String>).serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Pair>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (data0, data1): (Option<String>, Option<String>) =
            Deserialize::deserialize(deserializer)?;

        if let (Some(data0), Some(data1)) = (data0, data1) {
            Ok(Some(Pair(Address::from_str(&data0).unwrap(), Address::from_str(&data1).unwrap())))
        } else {
            Ok(None)
        }
    }
}
pub mod option_protocol {

    use std::str::FromStr;

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::Protocol;

    pub fn serialize<S: Serializer>(
        u: &Option<Protocol>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st = u.map(|u| u.to_string());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Protocol>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d: Option<String> = Deserialize::deserialize(deserializer)?;
        Ok(d.map(|d| Protocol::from_str(&d).unwrap()))
    }
}

pub mod vec_protocol {
    use std::str::FromStr;

    use itertools::Itertools;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::Protocol;

    pub fn serialize<S: Serializer>(u: &[Protocol], serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.iter().map(|u| u.to_string()).collect::<Vec<_>>();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Protocol>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d: Vec<String> = Deserialize::deserialize(deserializer)?;
        Ok(d.into_iter()
            .map(|d| Protocol::from_str(&d).unwrap())
            .collect_vec())
    }
}

pub mod cex_exchange {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::cex::CexExchange;

    pub fn serialize<S: Serializer>(u: &CexExchange, serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.to_string();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<CexExchange, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d: String = Deserialize::deserialize(deserializer)?;

        Ok(CexExchange::from(d.as_str()))
    }
}

pub mod cex_exchange_vec {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::cex::CexExchange;

    pub fn serialize<S: Serializer>(u: &[CexExchange], serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.iter().map(|u| u.to_string()).collect::<Vec<String>>();

        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<CexExchange>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d: Vec<String> = Deserialize::deserialize(deserializer)?;
        Ok(d.into_iter()
            .map(|d| CexExchange::from(d.as_str()))
            .collect::<_>())
    }
}
pub mod trade_type {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::db::cex::trades::TradeType;

    pub fn serialize<S: Serializer>(u: &TradeType, serializer: S) -> Result<S::Ok, S::Error> {
        let st = u.to_string();
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TradeType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d: String = Deserialize::deserialize(deserializer)?;

        Ok(match d.as_str() {
            "Maker" => TradeType::Maker,
            "Taker" => TradeType::Taker,
            _ => panic!("not maker or taker"),
        })
    }
}

pub mod u128_from_hex {
    use serde::de::Deserializer;

    pub fn deserialize_u128_from_hex<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First deserialize as a string
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;

        // Remove "0x" prefix if present
        let s = s.strip_prefix("0x").unwrap_or(&s);

        // Parse the hex string into u128
        u128::from_str_radix(s, 16).map_err(serde::de::Error::custom)
    }
}
