use std::{fmt::Debug, pin::Pin, str::FromStr, sync::Arc};

use brontes_pricing::{Protocol, SubGraphsEntry};
use brontes_types::{
    db::{
        address_to_tokens::PoolTokens, cex::CexPriceMap, dex::DexQuoteWithIndex,
        metadata::MetadataInner, mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses, traces::TxTracesInner,
    },
    extra_processing::Pair,
};
use futures::StreamExt;
use sorella_db_databases::Database;

use crate::libmdbx::{tables::traces::TxTracesData, types::dex_price::DexKey};

mod const_sql;
use alloy_primitives::Address;
use const_sql::*;
use futures::Future;
use paste::paste;
use reth_db::{table::Table, TableType};
use serde::Deserialize;
use sorella_db_databases::clickhouse::DbRow;

//use tracing::info;
use self::{
    address_to_tokens::LibmdbxPoolTokens, cex_price::LibmdbxCexPriceMap,
    dex_price::LibmdbxDexQuoteWithIndex, metadata::LibmdbxMetadataInner,
    mev_block::LibmdbxMevBlockWithClassified, pool_creation_block::LibmdbxPoolsToAddresses,
    subgraphs::LibmdbxSubGraphsEntry, traces::LibmdbxTxTracesInner,
};
//use crate::types::traces::TxTracesDBData;
use super::{
    types::{
        address_to_factory::AddressToFactoryData, address_to_protocol::AddressToProtocolData,
        address_to_tokens::AddressToTokensData, cex_price::CexPriceData, dex_price::DexPriceData,
        metadata::MetadataData, mev_block::MevBlocksData,
        pool_creation_block::PoolCreationBlocksData, subgraphs::SubGraphsData, *,
    },
    Libmdbx,
};
use crate::clickhouse::Clickhouse;

pub const NUM_TABLES: usize = 11;

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
    AddressToFactory,
    SubGraphs,
    TxTraces,
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
        Tables::AddressToFactory,
        Tables::SubGraphs,
        Tables::TxTraces,
    ];
    pub const ALL_NO_DEX: [Tables; NUM_TABLES - 3] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::PoolCreationBlocks,
        Tables::MevBlocks,
        Tables::Metadata,
        Tables::CexPrice,
        Tables::TxTraces,
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
            Tables::AddressToFactory => TableType::Table,
            Tables::SubGraphs => TableType::Table,
            Tables::TxTraces => TableType::Table,
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
            Tables::AddressToFactory => AddressToFactory::NAME,
            Tables::SubGraphs => SubGraphs::NAME,
            Tables::TxTraces => TxTraces::NAME,
        }
    }

    pub(crate) fn initialize_table<'a>(
        &'a self,
        libmdbx: Arc<Libmdbx>,
        // tracer: Arc<TracingClient>,
        clickhouse: Arc<Clickhouse>,
        _block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'a>> {
        match self {
            Tables::TokenDecimals => {
                println!("Starting {}", self.name());
                TokenDecimals::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::AddressToTokens => {
                println!("Starting {}", self.name());
                AddressToTokens::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::AddressToProtocol => {
                println!("Starting {}", self.name());
                AddressToProtocol::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::CexPrice => {
                //let block_range = (15400000, 19000000);
                println!("Starting {}", self.name());
                Box::pin(async move {
                    //libmdbx.clear_table::<CexPrice>()?;

                    //println!("Cleared Table: {}", CexPrice::NAME);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (15400000, 17800000),
                    )
                    .await?;
                    println!("Finished {} Block Range: {}-{}", CexPrice::NAME, 15400000, 16000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (16000000, 17000000),
                    )
                    .await?;
                    println!("Finished {} Block Range: {}-{}", CexPrice::NAME, 16000000, 17000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (17000000, 18000000),
                    )
                    .await?;
                    println!("Finished {} Block Range: {}-{}", CexPrice::NAME, 17000000, 18000000);
                    CexPrice::initialize_table_batching(
                        libmdbx.clone(),
                        clickhouse.clone(),
                        (18000000, 19000000),
                    )
                    .await?;
                    println!("Finished {} Block Range: {}-{}", CexPrice::NAME, 18000000, 19000000);
                    println!("{} OK", CexPrice::NAME);
                    Ok(())
                })
            }
            Tables::Metadata => {
                println!("Starting {}", self.name());
                Box::pin(async move {
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
                })
            }
            Tables::DexPrice => {
                println!("Starting {}", self.name());
                DexPrice::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::PoolCreationBlocks => {
                println!("Starting {}", self.name());
                PoolCreationBlocks::initialize_table(libmdbx.clone(), clickhouse.clone())
            }
            Tables::MevBlocks => {
                println!("Starting {}", self.name());
                Box::pin(
                    async move { libmdbx.initialize_table::<MevBlocks, MevBlocksData>(&vec![]) },
                )
            }
            Tables::AddressToFactory => {
                println!("Starting {}", self.name());
                Box::pin(async move {
                    libmdbx.initialize_table::<AddressToFactory, AddressToFactoryData>(&vec![])
                })
            }
            Tables::SubGraphs => {
                println!("Starting {}", self.name());
                Box::pin(
                    async move { libmdbx.initialize_table::<SubGraphs, SubGraphsData>(&vec![]) },
                )
            }
            Tables::TxTraces => {
                println!("Starting {}", self.name());
                Box::pin(async move { libmdbx.initialize_table::<TxTraces, TxTracesData>(&vec![]) })
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
            AddressToFactory::NAME => Ok(Tables::AddressToFactory),
            SubGraphs::NAME => Ok(Tables::SubGraphs),
            TxTraces::NAME => Ok(Tables::TxTraces),
            _ => Err("Unknown table".to_string()),
        }
    }
}

pub trait IntoTableKey<T, K, D> {
    fn into_key(value: T) -> K;
    fn into_table_data(key: T, value: T) -> D;
}

pub trait CompressedTable: reth_db::table::Table
where
    <Self as Table>::Value: From<<Self as CompressedTable>::DecompressedValue>
        + Into<<Self as CompressedTable>::DecompressedValue>,
{
    type DecompressedValue: Debug;
    const INIT_CHUNK_SIZE: Option<usize>;
}

/*

#[macro_export]
macro_rules! compressed_table {
    // WITH $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $init_chunk_size:expr, $value:ty = $($table:tt)*) => {

        impl CompressedTable for $table_name {
            type DecompressedValue = $compressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
        }

        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value | Some($init_chunk_size), $value = $($table)*);
    };

    // WITH $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $value:ty = $($table:tt)*) => {

        impl CompressedTable for $table_name {
            type DecompressedValue = $compressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = None;
        }

        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value | None, $value = $($table)*);
    };

    // WITHOUT $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $init_chunk_size:expr, $value:ty = $($table:tt)*) => {

        impl CompressedTable for $table_name {
            type DecompressedValue = $compressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = Some($init_chunk_size);
        }

        table!($(#[$docs])+ ( $table_name ) $key | $value | Some($init_chunk_size), $value = $($table)*);
    };

    // WITHOUT $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $value:ty = $($table:tt)*) => {

        impl CompressedTable for $table_name {
            type DecompressedValue = $compressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
        }

        table!($(#[$docs])+ ( $table_name ) $key | $value | None, $value = $($table)*);
    };
}


*/

#[macro_export]
macro_rules! compressed_table {
    // WITH $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $init_chunk_size:expr, $decompressed_value:ty = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value | Some($init_chunk_size), $decompressed_value = $($table)*);
    };

    // WITH $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $decompressed_value:ty = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value | None, $decompressed_value = $($table)*);
    };

    // WITHOUT $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $init_chunk_size:expr, $decompressed_value:ty = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $decompressed_value | Some($init_chunk_size), $decompressed_value = $($table)*);
    };

    // WITHOUT $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $decompressed_value:ty = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $decompressed_value | None, $decompressed_value = $($table)*);
    };
}

macro_rules! table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $init_chunk_size:expr, $decompressed_value:ty = $($table:tt)*) => {
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

            table!($table_name, $key, $compressed_value, $($table)*);
        }

        impl reth_db::table::Table for $table_name {
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $compressed_value;
        }

        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
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

compressed_table!(
    /// token address -> decimals
    ( TokenDecimals ) Address | u8 = False
);

compressed_table!(
    /// Address -> tokens in pool
    ( AddressToTokens ) Address | LibmdbxPoolTokens | PoolTokens = False
);

compressed_table!(
    /// Address -> Static protocol enum
    ( AddressToProtocol ) Address | Protocol = True
);

compressed_table!(
    /// block num -> cex prices
    ( CexPrice ) u64 | LibmdbxCexPriceMap | 200000,  CexPriceMap = False
);

compressed_table!(
    /// block num -> metadata
    ( Metadata ) u64 | LibmdbxMetadataInner | 200000,  MetadataInner = False
);

compressed_table!(
    /// block number concat tx idx -> cex quotes
    ( DexPrice ) DexKey | LibmdbxDexQuoteWithIndex | DexQuoteWithIndex = False
);

compressed_table!(
    /// block number -> pools created in block
    ( PoolCreationBlocks ) u64 | LibmdbxPoolsToAddresses | PoolsToAddresses = False
);

compressed_table!(
    /// block number -> mev block with classified mev
    ( MevBlocks ) u64 | LibmdbxMevBlockWithClassified | MevBlockWithClassified = False
);

compressed_table!(
    /// pair -> Vec<(block_number, entry)>
    ( SubGraphs ) Pair | LibmdbxSubGraphsEntry | SubGraphsEntry = False
);

compressed_table!(
    /// address -> factory
    ( AddressToFactory ) Address | Protocol = True
);

compressed_table!(
    /// block number -> tx traces
    ( TxTraces ) u64 | LibmdbxTxTracesInner | TxTracesInner = False
);

pub(crate) trait InitializeTable<'db, D>: CompressedTable + Sized + 'db
where
    D: LibmdbxData<Self> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    <Self as Table>::Value: From<<Self as CompressedTable>::DecompressedValue>
        + Into<<Self as CompressedTable>::DecompressedValue>,
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

            let data = data.unwrap_or(vec![]);
            println!("finished querying with {} entries", data.len());
            libmdbx.initialize_table(&data)
        })
    }

    fn initialize_table_batching(
        libmdbx: Arc<Libmdbx>,
        db_client: Arc<Clickhouse>,
        block_range: (u64, u64), // inclusive of start only TODO
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + 'db>> {
        Box::pin(async move {
            let chunk = 100000;
            let mut num_chunks = (block_range.1 - block_range.0) / chunk;

            let tasks = (block_range.0..block_range.1)
                .filter(|block| block % chunk == 0)
                .collect::<Vec<_>>();

            let mut data_stream = futures::stream::iter(tasks)
                .map(|block| {
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
                })
                .buffer_unordered(5);

            println!("chunks remaining: {num_chunks}");
            while let Some(val) = data_stream.next().await {
                let data = val?;
                num_chunks -= 1;
                println!("chunks remaining: {num_chunks}");

                println!("finished querying chunk {num_chunks} with {} entries", data.len());
                if !data.is_empty() {
                    libmdbx.write_table(&data)?;
                }
                println!("wrote chunk {num_chunks} to table");
            }

            Ok(())
        })
    }
}
