#![allow(non_upper_case_globals)]

use std::{fmt::Debug, pin::Pin, str::FromStr, sync::Arc};
mod const_sql;
use alloy_primitives::{Address, TxHash};
use brontes_database::clickhouse::Clickhouse;
use brontes_pricing::types::{PoolKey, PoolStateSnapShot};
use const_sql::*;
use futures::Future;
use reth_db::{
    table::{Table},
    TableType,
};
use serde::Deserialize;
use sorella_db_databases::Row;

use crate::{
    types::{
        address_to_protocol::{AddressToProtocolData, StaticBindingsDb},
        address_to_tokens::{AddressToTokensData, PoolTokens},
        cex_price::{CexPriceData, CexPriceMap},
        dex_price::{DexPriceData, DexQuoteWithIndex},
        metadata::{MetadataData, MetadataInner},
        pool_creation_block::{PoolCreationBlocksData, PoolsLibmdbx},
        pool_state::PoolStateData,
        *,
    },
    Libmdbx,
};

pub const NUM_TABLES: usize = 8;

#[derive(Clone, Debug)]
pub enum Tables {
    TokenDecimals,
    AddressToTokens,
    AddressToProtocol,
    CexPrice,
    Metadata,
    PoolState,
    DexPrice,
    PoolCreationBlocks,
}

impl Tables {
    pub const ALL: [Tables; NUM_TABLES] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::CexPrice,
        Tables::Metadata,
        Tables::PoolState,
        Tables::DexPrice,
        Tables::PoolCreationBlocks,
    ];
    pub const ALL_NO_DEX: [Tables; NUM_TABLES - 2] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::CexPrice,
        Tables::Metadata,
        Tables::PoolCreationBlocks,
    ];

    /// type of table
    pub(crate) const fn table_type(&self) -> TableType {
        match self {
            Tables::TokenDecimals => TableType::Table,
            Tables::AddressToTokens => TableType::Table,
            Tables::AddressToProtocol => TableType::Table,
            Tables::CexPrice => TableType::Table,
            Tables::Metadata => TableType::Table,
            Tables::PoolState => TableType::Table,
            Tables::DexPrice => TableType::Table,
            Tables::PoolCreationBlocks => TableType::Table,
        }
    }

    pub(crate) const fn name(&self) -> &str {
        match self {
            Tables::TokenDecimals => TokenDecimals::NAME,
            Tables::AddressToTokens => AddressToTokens::NAME,
            Tables::AddressToProtocol => AddressToProtocol::NAME,
            Tables::CexPrice => CexPrice::NAME,
            Tables::Metadata => Metadata::NAME,
            Tables::PoolState => PoolState::NAME,
            Tables::DexPrice => DexPrice::NAME,
            Tables::PoolCreationBlocks => PoolCreationBlocks::NAME,
        }
    }

    pub(crate) fn initialize_table<'a>(
        &'a self,
        libmdbx: &'a Libmdbx,
        clickhouse: Arc<&'a Clickhouse>,
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'a>> {
        match self {
            Tables::TokenDecimals => {
                TokenDecimals::initialize_table(libmdbx, clickhouse, block_range)
            }
            Tables::AddressToTokens => {
                AddressToTokens::initialize_table(libmdbx, clickhouse, block_range)
            }
            Tables::AddressToProtocol => {
                AddressToProtocol::initialize_table(libmdbx, clickhouse, block_range)
            }
            Tables::CexPrice => CexPrice::initialize_table(libmdbx, clickhouse, block_range),
            Tables::Metadata => Metadata::initialize_table(libmdbx, clickhouse, block_range),
            Tables::PoolState => PoolState::initialize_table(libmdbx, clickhouse, block_range),
            Tables::DexPrice => DexPrice::initialize_table(libmdbx, clickhouse, block_range),
            Tables::PoolCreationBlocks => {
                PoolCreationBlocks::initialize_table(libmdbx, clickhouse, block_range)
            }
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
            CexPrice::NAME => return Ok(Tables::CexPrice),
            Metadata::NAME => return Ok(Tables::Metadata),
            PoolState::NAME => return Ok(Tables::PoolState),
            DexPrice::NAME => return Ok(Tables::DexPrice),
            PoolCreationBlocks::NAME => return Ok(Tables::PoolCreationBlocks),
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

        impl<'db> InitializeTable<'db, paste::paste! {[<$table_name Data>]}> for $table_name {
            fn initialize_query() -> &'static str {
                paste::paste! {[<$table_name InitQuery>]}
            }
        }
    };
}

#[macro_export]
/// Macro to declare duplicate key value table.
macro_rules! dupsort {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | [$subkey:ty] $value:ty) => {
        table!(
            $(#[$docs])+
            ///
            #[doc = concat!("`DUPSORT` table with subkey being: [`", stringify!($subkey), "`]")]
            ( $table_name ) $key | $value
        );
        impl DupSort for $table_name {
            type SubKey = $subkey;
        }

        impl<'db> InitializeDupTable<'db, paste::paste! {[<$table_name Data>]}> for $table_name {
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

table!(
    /// block num -> cex prices
    ( CexPrice ) u64 | CexPriceMap
);

table!(
    /// block num -> metadata
    ( Metadata ) u64 | MetadataInner
);

table!(
    /// pool key -> pool state
    ( PoolState ) PoolKey | PoolStateSnapShot
);

table!(
    /// block number concat tx idx -> cex quotes
    ( DexPrice ) TxHash | DexQuoteWithIndex
);

table!(
    /// block number -> pools created in block
    ( PoolCreationBlocks ) u64 | PoolsLibmdbx
);

pub(crate) trait InitializeTable<'db, D>: reth_db::table::Table + Sized + 'db
where
    D: LibmdbxData<Self> + Row + for<'de> Deserialize<'de> + Send + Sync + Debug,
{
    fn initialize_query() -> &'static str;

    fn initialize_table(
        libmdbx: &'db Libmdbx,
        db_client: Arc<&'db Clickhouse>,
        _block_range: Option<(u64, u64)>, // inclusive of start only TODO
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'db>> {
        Box::pin(async move {
            // let query = Self::initialize_query();

            /*
            if query.is_empty() {
                println!("init empty");
                libmdbx.initialize_table::<_, D>(&vec![])?;
                return Ok(())
            }

            let data = if let Some((start, end)) = block_range {
                if query.contains('?') {
                    db_client
                        .inner()
                        .query_many::<D>(query, &(start, end))
                        .await
                } else {
                    db_client.inner().query_many::<D>(query, &()).await
                }
            } else {
                db_client.inner().query_many::<D>(query, &()).await
            };*/

            let data = db_client
                .inner()
                .query_many::<D>(Self::initialize_query(), &())
                .await;

            if data.is_err() {
                println!("{} {:?}", Self::NAME, data);
            } else {
                println!("{} OK", Self::NAME);
            }

            // println!("\n\nREG Data: {:?}\n\n", data);

            /*
                        let data = match data {
                            Ok(dd) =>  {
                                for d in &dd {
                                    println!("DATA: {:?}\n\n\n", d);
                                };
                                Ok(dd)
                            },
                            Err(e) => {println!("DB: ERROR: {:?}", e); Err(e)}
                        };
            */

            libmdbx.initialize_table(&data?)
        })
    }
}

pub(crate) trait InitializeDupTable<'db, D>: reth_db::table::DupSort + Sized + 'db
where
    D: LibmdbxData<Self> + Row + for<'de> Deserialize<'de> + Send + Sync + Debug,
{
    fn initialize_query() -> &'static str;

    fn initialize_table(
        libmdbx: &'db Libmdbx,
        db_client: Arc<&'db Clickhouse>,
        _block_range: Option<(u64, u64)>, // inclusive of start only TODO
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'db>> {
        Box::pin(async move {
            let data = db_client
                .inner()
                .query_many::<D>(Self::initialize_query(), &())
                .await;

            // println!("\n\nDUP Data: {:?}\n\n", data);

            /*
                        let data = match data {
                            Ok(dd) =>  {
                                for d in &dd {
                                    println!("DATA: {:?}\n\n\n", d);
                                };
                                Ok(dd)
                            },
                            Err(e) => {println!("DB: ERROR: {:?}", e); Err(e)}
                        };
            */

            libmdbx.initialize_table(&data?)
        })
    }
}
