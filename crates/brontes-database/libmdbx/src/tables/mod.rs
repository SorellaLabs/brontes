#![allow(non_upper_case_globals)]

use std::{fmt::Debug, pin::Pin, str::FromStr, sync::Arc};
use crate::Pair;

use paste::paste;
use sorella_db_databases::Database;

mod const_sql;
use alloy_primitives::{Address, TxHash};
use brontes_database::clickhouse::Clickhouse;
use const_sql::*;
use futures::{future::join_all, Future};
use reth_db::{table::Table, TableType};
use serde::Deserialize;
use sorella_db_databases::Row;
use tracing::info;

use crate::{
    types::{
        address_to_protocol::{AddressToProtocolData, StaticBindingsDb},
        address_to_tokens::{AddressToTokensData, PoolTokens},
        cex_price::{CexPriceData, CexPriceMap},
        dex_price::{DexPriceData, DexQuoteWithIndex},
        metadata::{MetadataData, MetadataInner},
        mev_block::{MevBlockWithClassified, MevBlocksData},
        pool_creation_block::{PoolCreationBlocksData, PoolsLibmdbx},
        subgraphs::{SubGraphsData, SubGraphsEntry},
        *,
    },
    Libmdbx,
};

pub const NUM_TABLES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tables {
    TokenDecimals,
    AddressToTokens,
    AddressToProtocol,
    CexPrice,
    Metadata,
    DexPrice,
    PoolCreationBlocks,
    MevBlocks,
}

impl Tables {
    pub const ALL: [Tables; NUM_TABLES] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::CexPrice,
        Tables::Metadata,
        Tables::DexPrice,
        Tables::PoolCreationBlocks,
        Tables::MevBlocks,
    ];
    pub const ALL_NO_DEX: [Tables; NUM_TABLES - 1] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::CexPrice,
        Tables::Metadata,
        Tables::PoolCreationBlocks,
        Tables::MevBlocks,
    ];

    /// type of table
    pub(crate) const fn table_type(&self) -> TableType {
        match self {
            Tables::TokenDecimals => TableType::Table,
            Tables::AddressToTokens => TableType::Table,
            Tables::AddressToProtocol => TableType::Table,
            Tables::CexPrice => TableType::Table,
            Tables::Metadata => TableType::Table,
            Tables::DexPrice => TableType::Table,
            Tables::PoolCreationBlocks => TableType::Table,
            Tables::MevBlocks => TableType::Table,
        }
    }

    pub(crate) const fn name(&self) -> &str {
        match self {
            Tables::TokenDecimals => TokenDecimals::NAME,
            Tables::AddressToTokens => AddressToTokens::NAME,
            Tables::AddressToProtocol => AddressToProtocol::NAME,
            Tables::CexPrice => CexPrice::NAME,
            Tables::Metadata => Metadata::NAME,
            Tables::DexPrice => DexPrice::NAME,
            Tables::PoolCreationBlocks => PoolCreationBlocks::NAME,
            Tables::MevBlocks => MevBlocks::NAME,
        }
    }

    pub(crate) fn initialize_table<'a>(
        &'a self,
        libmdbx: Arc<Libmdbx>,
        clickhouse: Arc<Clickhouse>,
        _block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'a>> {
        match self {
            Tables::TokenDecimals => {
                TokenDecimals::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::AddressToTokens => {
                AddressToTokens::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::AddressToProtocol => {
                AddressToProtocol::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::CexPrice => {
                //let block_range = (15400000, 19000000);

                Box::pin(async move {
                    libmdbx.clear_table::<CexPrice>()?;
                    println!("Cleared Table: {}", CexPrice::NAME);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (15400000, 16000000),
                    )
                    .await?;
                    info!(target: "brontes::init", "Finished {} Block Range: {}-{}", CexPrice::NAME, 15400000, 16000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (16000000, 17000000),
                    )
                    .await?;
                    info!(target: "brontes::init", "Finished {} Block Range: {}-{}", CexPrice::NAME, 16000000, 17000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (17000000, 18000000),
                    )
                    .await?;
                    info!(target: "brontes::init", "Finished {} Block Range: {}-{}", CexPrice::NAME, 17000000, 18000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (18000000, 19000000),
                    )
                    .await?;
                    info!(target: "brontes::init", "Finished {} Block Range: {}-{}", CexPrice::NAME, 18000000, 19000000);
                    println!("{} OK", CexPrice::NAME);
                    Ok(())
                })
            }
            Tables::Metadata => Box::pin(async move {
                libmdbx.clear_table::<Metadata>()?;
                println!("Cleared Table: {}", Metadata::NAME);

                Metadata::initialize_table_batching(
                    libmdbx.clone(),
                    clickhouse.clone(),
                    (15400000, 19000000),
                )
                .await?;
                println!("{} OK", Metadata::NAME);
                Ok(())
            }),
            Tables::DexPrice => DexPrice::initialize_table(libmdbx.clone(), clickhouse.clone()),
            Tables::PoolCreationBlocks => {
                PoolCreationBlocks::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::MevBlocks => {
                Box::pin(
                    async move { libmdbx.initialize_table::<MevBlocks, MevBlocksData>(&vec![]) },
                )
            }
        }
    }
}

impl FromStr for Tables {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            TokenDecimals::NAME => Ok(Tables::TokenDecimals),
            AddressToTokens::NAME => Ok(Tables::AddressToTokens),
            AddressToProtocol::NAME => Ok(Tables::AddressToProtocol),
            CexPrice::NAME => Ok(Tables::CexPrice),
            Metadata::NAME => Ok(Tables::Metadata),
            DexPrice::NAME => Ok(Tables::DexPrice),
            PoolCreationBlocks::NAME => Ok(Tables::PoolCreationBlocks),
            MevBlocks::NAME => Ok(Tables::MevBlocks),
            _ => Err("Unknown table".to_string()),
        }
    }
}

pub trait IntoTableKey<T, K, D> {
    fn into_key(value: T) -> K;
    fn into_table_data(key: T, value: T) -> D;
}

/// Macro to declare key value table + extra impl
#[macro_export]
macro_rules! table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $value:ty = $($table:tt)*) => {
        $(#[$docs])+
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`].")]
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $table_name;

        impl IntoTableKey<&str, $key, paste!([<$table_name Data>])> for $table_name {
            fn into_key(value: &str) -> $key {
                let key: $key = value.parse().unwrap();
                println!("decoded key: {key:?}");
                key
            }

            table!($table_name, $key, $value, $($table)*);
        }

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
                paste! {[<$table_name InitQuery>]}
            }
        }
    };
    ($table_name:ident, $key:ty, $value:ty, True) => {
        fn into_table_data(key: &str, value: &str) -> paste!([<$table_name Data>]) {
            let key: $key = key.parse().unwrap();
            let value: $value = value.parse().unwrap();
            <paste!([<$table_name Data>])>::new(key, value)
        }

    };
    ($table_name:ident, $key:ty, $value:ty, False) => {
        fn into_table_data(_: &str, _: &str) -> paste!([<$table_name Data>]) {
            panic!("inserts not supported for $table_name");
        }
    }
}

table!(
    /// token address -> decimals
    ( TokenDecimals ) Address | u8 = False
);

table!(
    /// Address -> tokens in pool
    ( AddressToTokens ) Address | PoolTokens = False
);

table!(
    /// Address -> Static protocol enum
    ( AddressToProtocol ) Address | StaticBindingsDb = True
);

table!(
    /// block num -> cex prices
    ( CexPrice ) u64 | CexPriceMap = False
);

table!(
    /// block num -> metadata
    ( Metadata ) u64 | MetadataInner = False
);

table!(
    /// block number concat tx idx -> cex quotes
    ( DexPrice ) TxHash | DexQuoteWithIndex = False
);

table!(
    /// block number -> pools created in block
    ( PoolCreationBlocks ) u64 | PoolsLibmdbx = False
);

table!(
    /// block number -> mev block with classified mev
    ( MevBlocks ) u64 | MevBlockWithClassified = False
);


table!(
    /// pair -> Vec<(block_number, entry)>
    ( SubGraphs ) Pair | SubGraphsEntry = False
);


pub(crate) trait InitializeTable<'db, D>: reth_db::table::Table + Sized + 'db
where
    D: LibmdbxData<Self> + Row + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    fn initialize_query() -> &'static str;

    fn initialize_table(
        libmdbx: Arc<Libmdbx>,
        db_client: Arc<Clickhouse>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'db>> {
        Box::pin(async move {
            let data = db_client
                .inner()
                .query_many::<D>(Self::initialize_query(), &())
                .await;

            if data.is_err() {
                println!("{} ERROR - {:?}", Self::NAME, data);
            } else {
                println!("{} OK", Self::NAME);
            }

            libmdbx.initialize_table(&data?)
        })
    }

    fn initialize_table_batching(
        libmdbx: Arc<Libmdbx>,
        db_client: Arc<Clickhouse>,
        block_range: (u64, u64), // inclusive of start only TODO
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'db>> {
        Box::pin(async move {
            /*
                        let block_chunks = [
                            (15000000, 16000000),
                            (16000000, 16250000),
                            (16250000, 16500000),
                            (16500000, 16750000),
                            (16750000, 17000000),
                            (17000000, 17250000),
                            (17250000, 17500000),
                            (17500000, 17750000),
                            (17750000, 18000000),
                            (18000000, 18250000),
                            (18250000, 18500000),
                            (18500000, 18750000),
                            (18750000, 19000000),
                        ];

                        let data = join_all(block_chunks.into_iter().map(|params| {
                            let db_client = db_client.clone();
                            async move {
                                db_client
                                    .inner()
                                    .query_many::<D>(Self::initialize_query(), &params)
                                    .await
                            }
                        }))
                        .await
                        .into_iter()
                        //.flatten()
                        .collect::<Result<Vec<_>, _>>();
            */

            let chunk = 10000;
            let tasks = (block_range.0..block_range.1)
                .filter(|block| block % chunk == 0)
                .collect::<Vec<_>>();

            let data = //futures::stream::iter(tasks)
                join_all(tasks.into_iter().map(|block| {
                    let db_client = db_client.clone();
                    tokio::spawn(async move {
                        let data = db_client
                            .inner()
                            .query_many::<D>(Self::initialize_query(), &(block - chunk, block))
                            .await;

                        if data.is_err() {
                            println!(
                                "{} Block Range: {} - {} --- ERROR: {:?}",
                                Self::NAME,
                                block - chunk,
                                block,
                                data,
                            );
                        }

                        data.unwrap()
                    })
                })).await.into_iter().collect::<Result<Vec<_>, _>>()?.into_iter().flatten().collect::<Vec<_>>();

            libmdbx.write_table(&data)?;
            /* .buffer_unordered(50);

            while let Some(d) = data.next().await {
                let data_des = d?;

                libmdbx.write_table(&data_des)?;
                //drop(data_des);
            }*/

            Ok(())
        })
    }
}
