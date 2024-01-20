#![allow(non_uppeRedefined_case_globals)]

use std::{fmt::Debug, pin::Pin, str::FromStr, sync::Arc};

use brontes_types::{traits::TracingProvider, exchanges::StaticBindingsDb};
use futures::StreamExt;
use reth_interfaces::blockchain_tree::BlockchainTreeViewer;
use reth_primitives::{BlockId, BlockNumberOrTag};
use reth_rpc::eth::error::EthApiError;
use sorella_db_databases::Database;

use crate::Pair;

mod const_sql;
use alloy_primitives::{Address, TxHash};
use brontes_database::clickhouse::Clickhouse;
use const_sql::*;
use futures::{future::join_all, Future};
use paste::paste;
use reth_db::{table::Table, TableType};
use serde::Deserialize;
use sorella_db_databases::clickhouse::DbRow;
use tracing::info;

use self::{
    pool_creation_block::PoolsToAddresses,
    redefined_types::{
        address_to_tokens::Redefined_PoolTokens, cex_price::Redefined_CexPriceMap,
        dex_price::Redefined_DexQuoteWithIndex, metadata::Redefined_MetadataInner,
        pool_creation_block::Redefined_PoolsToAddresses, subgraph::Redefined_SubGraphsEntry, mev_block::Redefined_MevBlockWithClassified,
    },
};
//use crate::types::traces::TxTracesDBData;
use crate::{
    types::{
        address_to_factory::AddressToFactoryData,
        address_to_protocol::{AddressToProtocolData},
        address_to_tokens::{AddressToTokensData, PoolTokens},
        cex_price::{CexPriceData, CexPriceMap},
        dex_price::{DexPriceData, DexQuoteWithIndex},
        metadata::{MetadataData, MetadataInner},
        mev_block::{MevBlockWithClassified, MevBlocksData},
        pool_creation_block::PoolCreationBlocksData,
        subgraphs::{SubGraphsData, SubGraphsEntry},
        *,
    },
    Libmdbx,
};

pub const NUM_TABLES: usize = 10;

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
    ];
    pub const ALL_NO_DEX: [Tables; NUM_TABLES - 3] = [
        Tables::TokenDecimals,
        Tables::AddressToTokens,
        Tables::AddressToProtocol,
        Tables::CexPrice,
        Tables::Metadata,
        Tables::PoolCreationBlocks,
        Tables::MevBlocks,
        // Tables::TxTraces
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
            Tables::AddressToFactory => Box::pin(async move {
                libmdbx.initialize_table::<AddressToFactory, AddressToFactoryData>(&vec![])
            }),
            Tables::SubGraphs => {
                Box::pin(
                    async move { libmdbx.initialize_table::<SubGraphs, SubGraphsData>(&vec![]) },
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
            AddressToFactory::NAME => Ok(Tables::AddressToFactory),
            SubGraphs::NAME => Ok(Tables::SubGraphs),
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
    <Self as Table>::Value: From<<Self as CompressedTable>::DecompressedValue> +  Into<<Self as CompressedTable>::DecompressedValue>,
{
    type DecompressedValue;
}

//  $compressed_value:ty |
/// Macro to declare key value table + extra impl

#[macro_export]
macro_rules! compressed_table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ty | $value:ty = $($table:tt)*) => {
        impl CompressedTable for $table_name {
            type DecompressedValue = $value;
        }

        table!(
            /// token address -> decimals
            ( $table_name ) $key | $compressed_value = $($table)*
        );
    };

    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $value:ty = $($table:tt)*) => {
        impl CompressedTable for $table_name {
            type DecompressedValue = $value;
        }

        table!(
            /// token address -> decimals
            ( $table_name ) $key | $value = $($table)*
        );
    };
}

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

compressed_table!(
    /// token address -> decimals
    ( TokenDecimals ) Address | u8 = False
);

compressed_table!(
    /// Address -> tokens in pool
    ( AddressToTokens ) Address | Redefined_PoolTokens | PoolTokens = False
);

compressed_table!(
    /// Address -> Static protocol enum
    ( AddressToProtocol ) Address | StaticBindingsDb = True
);

compressed_table!(
    /// block num -> cex prices
    ( CexPrice ) u64 | Redefined_CexPriceMap | CexPriceMap = False
);

compressed_table!(
    /// block num -> metadata
    ( Metadata ) u64 | Redefined_MetadataInner | MetadataInner = False
);

compressed_table!(
    /// block number concat tx idx -> cex quotes
    ( DexPrice ) TxHash | Redefined_DexQuoteWithIndex | DexQuoteWithIndex = False
);

compressed_table!(
    /// block number -> pools created in block
    ( PoolCreationBlocks ) u64 | Redefined_PoolsToAddresses | PoolsToAddresses = False
);

compressed_table!(
    /// block number -> mev block with classified mev
    ( MevBlocks ) u64 | Redefined_MevBlockWithClassified | MevBlockWithClassified = False
);

compressed_table!(
    /// pair -> Vec<(block_number, entry)>
    ( SubGraphs ) Pair | Redefined_SubGraphsEntry | SubGraphsEntry = False
);

compressed_table!(
    /// address -> factory
    ( AddressToFactory ) Address | StaticBindingsDb = True
);

pub(crate) trait InitializeTable<'db, D>: CompressedTable + Sized + 'db
where
    D: LibmdbxData<Self> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    <Self as Table>::Value: From<<Self as CompressedTable>::DecompressedValue> + Into<<Self as CompressedTable>::DecompressedValue>,
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

            let chunk = 100000;
            let mut num_chunks = (block_range.1 - block_range.0) / chunk;

            let tasks = (block_range.0..block_range.1)
                .filter(|block| block % chunk == 0)
                .collect::<Vec<_>>();

            
            let mut data_stream = //futures::stream::iter(tasks)
                futures::stream::iter(tasks).map(|block| {
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
                }).buffer_unordered(5);


                let mut data = Vec::new();
                println!("chunks remaining: {num_chunks}");
                while let Some(val) = data_stream.next().await {
                    data.extend(val?);
                    num_chunks -= 1;
                    println!("chunks remaining: {num_chunks}");
                }
                
                //.await.into_iter().collect::<Result<Vec<_>, _>>()?.into_iter().flatten().collect::<Vec<_>>();

                println!("finished querying");
                if !data.is_empty() {
                    libmdbx.write_table(&data)?;
                }


            /* .buffeRedefined_unordered(50);

            while let Some(d) = data.next().await {
                let data_des = d?;

                libmdbx.write_table(&data_des)?;
                //drop(data_des);
            }*/

            Ok(())
        })
    }
}

/*

impl TxTracesDB {
    pub async fn initialize_table_node(libmdbx: Arc<Libmdbx>, tracer: Arc<TracingClient>) -> eyre::Result<()> {
        let start_block: u64 = 15400000;
        let current_block = tracer.api.provider().canonical_tip().number;

        libmdbx.cleaRedefined_table::<TxTracesDB>()?;

        let range = (start_block..current_block).collect::<Vec<_>>();
    let chunks = range.chunks(1000).collect::<Vec<_>>();
    let tracer = tracer.as_ref();
        let mut tx_traces_stream = futures::stream::iter(chunks).map(|chunk| {
            join_all( chunk.into_iter().map(|block_num|

                async move {
                    Ok(TxTracesDBData::new(*block_num, TxTracesInner::new(tracer.replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(*block_num))).await?)))
                }
         ) )}).buffeRedefined_unordered(1);

       // .await

       while let Some(result) = tx_traces_stream.next().await {
        let tx_traces = result.into_iter().collect::<Result<Vec<_>, EthApiError>>()?;
        libmdbx.write_table(&tx_traces)?;
        println!("FINISHED CHUNK: {:?} - {:?}", tx_traces.first().map(|val| val.block_number), tx_traces.last().map(|val| val.block_number));
       }



    Ok(())

    }
}*/
