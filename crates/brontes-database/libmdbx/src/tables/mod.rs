#![allow(non_upper_case_globals)]

use std::{pin::Pin, str::FromStr};
mod const_sql;
use alloy_primitives::Address;
use brontes_database::clickhouse::Clickhouse;
use const_sql::*;
use futures::Future;
use reth_db::{dupsort, table::Table, TableType};
use serde::Deserialize;
use sorella_db_databases::Row;

use crate::{
    types::{
        address_to_protocol::{AddressToProtocolData, StaticBindingsDb},
        address_to_tokens::{AddressToTokensData, PoolTokens},

        *,
    },
    Libmdbx,
};

pub const NUM_TABLES: usize = 3;

pub enum Tables {
    TokenDecimals,
    AddressToTokens,
    AddressToProtocol,
}

impl Tables {
    pub const ALL: [Tables; NUM_TABLES] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
    ];

    /// type of table
    pub(crate) const fn table_type(&self) -> TableType {
        match self {
            Tables::TokenDecimals => TableType::Table,
            Tables::AddressToTokens => TableType::Table,
            Tables::AddressToProtocol => TableType::Table,
        }
    }

    pub(crate) const fn name(&self) -> &str {
        match self {
            Tables::TokenDecimals => TokenDecimals::NAME,
            Tables::AddressToTokens => AddressToTokens::NAME,
            Tables::AddressToProtocol => AddressToProtocol::NAME,
        }
    }

    pub(crate) fn initialize_table<'a>(
        &'a self,
        libmdbx: &'a Libmdbx,
        clickhouse: &'a Clickhouse,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'a>> {
        match self {
            Tables::TokenDecimals => TokenDecimals::initialize_table(libmdbx, clickhouse),
            Tables::AddressToTokens => AddressToTokens::initialize_table(libmdbx, clickhouse),
            Tables::AddressToProtocol => AddressToProtocol::initialize_table(libmdbx, clickhouse),
        }
    }
}

impl FromStr for Tables {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            TokenDecimals::NAME => return Ok(Tables::TokenDecimals),
            AddressToTokens::NAME => return Ok(Tables::AddressToTokens),
            AddressToProtocol::NAME => return Ok(Tables::AddressToProtocol),
            _ => return Err("Unknown table".to_string()),
        }
    }
}

/// Macro to declare key value table + extra impl
#[macro_export]
macro_rules! table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $value:ty) => {
        $(#[$docs])+
        ///
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`].")]
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $table_name;

        impl reth_db::table::Table for $table_name {
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $value;
        }

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
            }
        }

        impl<'fut, 'db: 'fut> InitializeTable<'fut, 'db, paste::paste! {[<$table_name Data>]}> for $table_name {
            fn initialize_query() -> &'static str {
                paste::paste! {[<$table_name InitQuery>]}
            }
        }
    };
}

table!(
    /// token address -> decimals
    ( TokenDecimals ) Address | u8
);

table!(
    /// Address -> tokens in pool
    ( AddressToTokens ) Address | PoolTokens
);

table!(
    /// Address -> Static protocol enum
    ( AddressToProtocol ) Address | StaticBindingsDb
);

pub(crate) trait InitializeTable<'fut, 'db: 'fut, D>:
    reth_db::table::Table + Sized + 'db
where
    D: LibmdbxData<Self> + Row + for<'de> Deserialize<'de> + Send + Sync,
{
    fn initialize_query() -> &'static str;

    fn initialize_table(
        libmdbx: &'db Libmdbx,
        db_client: &'db Clickhouse,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'fut>> {
        Box::pin(async move {
            let data = db_client
                .inner()
                .query_many::<D>(Self::initialize_query(), &())
                .await?;
            libmdbx.initialize_table(&data)
        })
    }
}
