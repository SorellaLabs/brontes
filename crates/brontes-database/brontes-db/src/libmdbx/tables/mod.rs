use std::{
    fmt::{Debug, Display},
    future::Future,
    pin::Pin,
    str::FromStr,
    sync::Arc,
};

use brontes_types::{
    db::{
        address_metadata::{AddressMetadata, AddressMetadataRedefined},
        address_to_protocol_info::{ProtocolInfo, ProtocolInfoRedefined},
        builder::{BuilderInfo, BuilderInfoRedefined},
        cex::{
            quotes::{CexPriceMap, CexPriceMapRedefined},
            trades::{CexTradeMap, CexTradeMapRedefined},
        },
        clickhouse_serde::tx_trace::tx_traces_inner,
        dex::{DexKey, DexQuoteWithIndex, DexQuoteWithIndexRedefined},
        initialized_state::{
            InitializedStateMeta, CEX_QUOTES_FLAG, CEX_TRADES_FLAG, DEX_PRICE_FLAG, META_FLAG,
            TRACE_FLAG,
        },
        metadata::{BlockMetadataInner, BlockMetadataInnerRedefined},
        mev_block::{MevBlockWithClassified, MevBlockWithClassifiedRedefined},
        pool_creation_block::{PoolsToAddresses, PoolsToAddressesRedefined},
        searcher::{SearcherInfo, SearcherInfoRedefined},
        token_info::TokenInfo,
        traces::{TxTracesInner, TxTracesInnerRedefined},
        traits::LibmdbxReader,
    },
    serde_utils::*,
    traits::TracingProvider,
};
use clickhouse::DbRow;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use reth_db::table::Table;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{types::ReturnKV, utils::protocol_info, LibmdbxData, LibmdbxReadWriter},
    parquet::ParquetExporter,
};
mod const_sql;
use alloy_primitives::Address;
//
// use brontes_types::db::initialized_state::CEX_QUOTES_FLAG;
//
// use brontes_types::db::initialized_state::CEX_TRADES_FLAG;
use const_sql::*;
use paste::paste;
use reth_db::TableType;

use super::{
    initialize::LibmdbxInitializer, libmdbx_writer::WriterMessage, types::IntoTableKey,
    CompressedTable,
};

pub const NUM_TABLES: usize = 14;

macro_rules! tables {
    ($($table:ident),*) => {
        #[derive(Debug, PartialEq, Copy, Clone, Eq, Hash, Serialize, Deserialize)]
        #[repr(u8)]
        /// Default tables that should be present inside database.
        pub enum Tables {
            $(
                #[doc = concat!("Represents a ", stringify!($table), " table")]
                $table,
            )*
        }

        impl Tables {
            /// Array of all tables in database
            pub const ALL: [Tables; NUM_TABLES] = [$(Tables::$table,)*];

            /// The name of the given table in database
            pub const fn name(&self) -> &str {
                match self {
                    $(Tables::$table => {
                        $table::NAME
                    },)*
                }
            }

            /// The type of the given table in database
            pub const fn table_type(&self) -> TableType {
                match self {
                    $(Tables::$table => {
                        TableType::Table
                    },)*
                }
            }

            pub fn init_table(&self, db: &LibmdbxReadWriter) -> eyre::Result<()> {
                match self {
                    $(
                        Tables::$table => {
                            let tx = db.db.rw_tx()?;
                            tx.get_dbi::<$table>()?;
                            tx.commit()?;
                        }
                    ),*
                }

                Ok(())
            }

        }

        impl Display for Tables {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.name())
            }
        }

        impl FromStr for Tables {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($table::NAME => {
                        Ok(Tables::$table)
                    },)*
                    _ => {
                        Err("Unknown table".to_string())
                    }
                }
            }
        }
    };
}

impl Tables {
    pub fn build_init_state_progress_bar(
        &self,
        multi_progress_bar: &MultiProgress,
        blocks_to_init: u64,
    ) -> ProgressBar {
        let progress_bar = ProgressBar::with_draw_target(
            Some(blocks_to_init),
            ProgressDrawTarget::stderr_with_hz(50),
        );
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{elapsed_precise}] [{wide_bar:.green/red}] {pos}/{len} ({percent}%)",
            )
            .unwrap()
            .progress_chars("#>-")
            .with_key(
                "percent",
                |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                },
            ),
        );
        progress_bar.set_message(format!("{}", self));
        multi_progress_bar.add(progress_bar)
    }

    pub(crate) async fn initialize_full_range_table<T: TracingProvider, CH: ClickhouseHandle>(
        &self,
        initializer: &LibmdbxInitializer<T, CH>,
        crit_progress: ProgressBar,
    ) -> eyre::Result<()> {
        let handle = initializer.get_libmdbx_handle();
        match self {
            Tables::TokenDecimals => {
                initializer
                    .clickhouse_init_no_args::<TokenDecimals, TokenDecimalsData>(
                        crit_progress,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::AddressToProtocolInfo => {
                initializer
                    .clickhouse_init_no_args::<AddressToProtocolInfo, AddressToProtocolInfoData>(
                        crit_progress,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::PoolCreationBlocks => {
                initializer
                    .clickhouse_init_no_args::<PoolCreationBlocks, PoolCreationBlocksData>(
                        crit_progress,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::Builder => {
                initializer
                    .clickhouse_init_no_args::<Builder, BuilderData>(crit_progress, |f, not| {
                        handle.send_message(WriterMessage::Init(f.into(), not))
                    })
                    .await
            }
            Tables::AddressMeta => {
                initializer
                    .clickhouse_init_no_args::<AddressMeta, AddressMetaData>(
                        crit_progress,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            _ => unimplemented!("{:?} isn't a full range table", self),
        }
    }

    pub(crate) async fn initialize_table<T: TracingProvider, CH: ClickhouseHandle>(
        &self,
        initializer: &LibmdbxInitializer<T, CH>,
        block_range: Option<(u64, u64)>,
        clear_table: bool,
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        let handle = initializer.get_libmdbx_handle();
        match self {
            Tables::CexPrice => {
                initializer
                    .initialize_table_from_clickhouse::<CexPrice, CexPriceData>(
                        block_range,
                        clear_table,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_range_quotes,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::BlockInfo => {
                initializer
                    .initialize_table_from_clickhouse::<BlockInfo, BlockInfoData>(
                        block_range,
                        clear_table,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_range,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::DexPrice => {
                initializer
                    .initialize_table_from_clickhouse::<DexPrice, DexPriceData>(
                        block_range,
                        clear_table,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_range,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::MevBlocks => Ok(()),
            Tables::TxTraces => {
                initializer
                    .initialize_table_from_clickhouse::<TxTraces, TxTracesData>(
                        block_range,
                        clear_table,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_range,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }

            Tables::CexTrades => {
                initializer
                    .initialize_table_from_clickhouse::<CexTrades, CexTradesData>(
                        block_range,
                        clear_table,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_range_trades,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::SearcherEOAs | Tables::SearcherContracts | Tables::InitializedState => Ok(()),
            _ => unimplemented!("'initialize_table' not implemented for {:?}", self),
        }
    }

    pub(crate) async fn initialize_table_arbitrary_state<
        T: TracingProvider,
        CH: ClickhouseHandle,
    >(
        &self,
        initializer: &LibmdbxInitializer<T, CH>,
        block_range: &'static [u64],
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        let handle = initializer.get_libmdbx_handle();
        match self {
            Tables::CexPrice => {
                initializer
                    .initialize_table_from_clickhouse_arbitrary_state::<CexPrice, CexPriceData>(
                        block_range,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_arbitrary_quotes,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::BlockInfo => {
                initializer
                    .initialize_table_from_clickhouse_arbitrary_state::<BlockInfo, BlockInfoData>(
                        block_range,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_arbitrary,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::DexPrice => {
                initializer
                    .initialize_table_from_clickhouse_arbitrary_state::<DexPrice, DexPriceData>(
                        block_range,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_arbitrary,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            Tables::TxTraces => {
                initializer
                    .initialize_table_from_clickhouse_arbitrary_state::<TxTraces, TxTracesData>(
                        block_range,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_arbitrary,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }

            Tables::CexTrades => {
                initializer
                    .initialize_table_from_clickhouse_arbitrary_state::<CexTrades, CexTradesData>(
                        block_range,
                        self.fetch_progress_bar(progress_bar),
                        Self::fetch_download_fn_arbitrary_trades,
                        |f, not| handle.send_message(WriterMessage::Init(f.into(), not)),
                    )
                    .await
            }
            table @ (Tables::TokenDecimals
            | Tables::AddressToProtocolInfo
            | Tables::PoolCreationBlocks
            | Tables::Builder
            | Tables::AddressMeta) => {
                unimplemented!("'initialize_table_arbitrary_state' not implemented for {}", table);
            }
            _ => Ok(()),
        }
    }

    fn fetch_progress_bar(&self, progress_bar: Arc<Vec<(Tables, ProgressBar)>>) -> ProgressBar {
        progress_bar
            .iter()
            .find_map(|(t, b)| (t == self).then_some(b.clone()))
            .unwrap()
    }

    pub async fn export_to_parquet<DB>(
        &self,
        exporter: Arc<ParquetExporter<DB>>,
    ) -> eyre::Result<()>
    where
        DB: LibmdbxReader,
    {
        match self {
            Self::AddressMeta => exporter.export_address_metadata().await,
            Self::MevBlocks => exporter.export_mev_blocks().await,
            Self::SearcherContracts | Self::SearcherEOAs => exporter.export_searcher_info().await,
            Self::Builder => exporter.export_builder_info().await,
            _ => unreachable!("Parquet export not yet supported for this table"),
        }
    }

    fn fetch_download_fn_range<CH: ClickhouseHandle, T, D>(
        start: u64,
        end: u64,
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move { ch.query_many_range::<T, D>(start, end).await })
    }

    fn fetch_download_fn_range_trades<CH: ClickhouseHandle, T, D>(
        start: u64,
        end: u64,
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move {
            unsafe {
                std::mem::transmute(
                    ch.get_cex_trades(super::cex_utils::CexRangeOrArbitrary::Range(start, end))
                        .await,
                )
            }
        })
    }

    pub fn fetch_download_fn_range_quotes<CH: ClickhouseHandle, T, D>(
        start: u64,
        end: u64,
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move {
            unsafe {
                std::mem::transmute(
                    ch.get_cex_prices(super::cex_utils::CexRangeOrArbitrary::Range(start, end))
                        .await,
                )
            }
        })
    }

    fn fetch_download_fn_arbitrary<CH: ClickhouseHandle, T, D>(
        range: &'static [u64],
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move { ch.query_many_arbitrary::<T, D>(range).await })
    }

    fn fetch_download_fn_arbitrary_trades<CH: ClickhouseHandle, T, D>(
        range: &'static [u64],
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move {
            unsafe {
                std::mem::transmute(
                    ch.get_cex_trades(super::cex_utils::CexRangeOrArbitrary::Arbitrary(range))
                        .await,
                )
            }
        })
    }

    fn fetch_download_fn_arbitrary_quotes<CH: ClickhouseHandle, T, D>(
        range: &'static [u64],
        ch: &'static CH,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<D>>> + Send>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        Box::pin(async move {
            unsafe {
                std::mem::transmute(
                    ch.get_cex_prices(super::cex_utils::CexRangeOrArbitrary::Arbitrary(range))
                        .await,
                )
            }
        })
    }
}

tables!(
    TokenDecimals,
    AddressToProtocolInfo,
    CexPrice,
    BlockInfo,
    DexPrice,
    PoolCreationBlocks,
    MevBlocks,
    TxTraces,
    Builder,
    AddressMeta,
    SearcherEOAs,
    SearcherContracts,
    InitializedState,
    CexTrades
);

/// Must be in this order when defining
/// Table {
///     Data {
///     },
///     Init {
///     }
///     CLI
///     }
/// }
macro_rules! compressed_table {
    (Table $(#[$attrs:meta])* $table_name:ident { $($head:tt)* }) => {
        compressed_table!($(#[$attrs:meta])* $table_name {} $($head)*);
    };
    (
        $(#[$attrs:meta])* $table_name:ident,
        $c_val:ident, $decompressed_value:ident, $key:ident
        { $($acc:tt)* } $(,)*
    ) => {
        $(#[$attrs])*
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`].")]
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $table_name;

        impl reth_db::table::Table for $table_name {
            // this type is needed for the trait impl but we never actually use it,
            // so an arbitrary table will do
            const TABLE: reth_db::Tables = reth_db::Tables::CanonicalHeaders;
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $c_val;
        }

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
            }
        }

        #[cfg(feature = "tests")]
        #[allow(unused)]
        impl $table_name {
            pub async fn test_initialized_data<CH: ClickhouseHandle>(
                clickhouse: &CH,
                libmdbx: &crate::libmdbx::LibmdbxReadWriter,
                block_range: Option<(u64, u64)>
            ) -> eyre::Result<()> {
                paste::paste!{
                    crate::libmdbx::test_utils::compare_clickhouse_libmdbx_data
                    ::<$table_name,[<$table_name Data>], CH>(clickhouse, libmdbx, block_range).await
                }
            }

            pub async fn test_initialized_arbitrary_data<CH: ClickhouseHandle>(
                clickhouse: &CH,
                libmdbx: &crate::libmdbx::LibmdbxReadWriter,
                block_range: &'static [u64]
            ) -> eyre::Result<()> {
                paste::paste!{
                    crate::libmdbx::test_utils::compare_clickhouse_libmdbx_arbitrary_data
                    ::<$table_name,[<$table_name Data>], CH>(clickhouse, libmdbx, block_range).await
                }
            }
        }

        $($acc)*
    };
    (
        $(#[$attrs:meta])* $table_name:ident
        { $($acc:tt)* } $(#[$dattrs:meta])*
        Data {
            $(#[$kattrs:meta])* key: $key:ident,
            $(#[$vattrs:meta])* value: $val:ident,
            $(#[$vcattrs:meta])* compressed_value: $c_val:ident
        },  $($tail:tt)*
    ) => {
        compressed_table!($(#[$attrs])* $table_name { $($acc)* }
                          Data  {
                              $(#[$kattrs])* key: $key,
                              $(#[$vattrs])* value: $val,
                              $(#[$vcattrs])* compressed_value: $c_val false
                          }, $($tail)*);

    };
    // parse key value compressed
    ($(#[$attrs:meta])* $table_name:ident
     { $($acc:tt)* } $(#[$dattrs:meta])*
        Data {
            $(#[$kattrs:meta])* key: $key:ident,
            $(#[$vattrs:meta])* value: $val:ident,
            $(#[$vcattrs:meta])* compressed_value: $c_val:ident false
        },  $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $val, $key {
        $($acc)*
        paste!(
        #[derive(Debug, Clone, Default, clickhouse::Row, serde::Serialize, serde::Deserialize)]
        $(#[$dattrs])*
        pub struct [<$table_name Data>] {
            $(#[$kattrs])*
            pub key: $key,
            $(#[$vattrs])*
            pub value: $val
        }

        impl [<$table_name Data>] {
            pub fn new(key: $key, value: $val) -> Self {
                [<$table_name Data>] {
                    key,
                    value
                }
            }
        }

        impl From<($key, $val)> for [<$table_name Data>] {
            fn from(value: ($key, $val)) -> Self {
                [<$table_name Data>] {
                    key: value.0,
                    value: value.1
                }
            }
        }

        impl LibmdbxData<$table_name> for [<$table_name Data>] {
            fn into_key_val(&self) -> ReturnKV<$table_name> {
                (self.key.clone(), self.value.clone()).into()
            }
        }

        );
    } $($tail)*);
    };
    ($(#[$attrs:meta])* $table_name:ident { $($acc:tt)* } $(#[$dattrs:meta])*
     Data {
         $(#[$kattrs:meta])* key: $key:ident,
         $(#[$vattrs:meta])* value: $val:ident
     },  $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name { $($acc)* } $(#[$dattrs])*
                          Data  {
                              $(#[$kattrs])* key: $key,
                              $(#[$vattrs])* value: $val,
                              $(#[$vattrs])* compressed_value: $val false
                          }, $($tail)*);

    };
    ($(#[$attrs:meta])* $table_name:ident, $c_val:ident, $decompressed_value:ident, $key:ident
     { $($acc:tt)* } Init { init_size: $init_chunk_size:expr, init_method: Clickhouse,
                              http_endpoint: $http_endpoint:expr, init_flag: $init_flag:expr },

     $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = Some(paste! {[<$table_name InitQuery>]});
            const HTTP_ENDPOINT: Option<&'static str> = $http_endpoint;
            const INIT_FLAG: Option<u8> = $init_flag;
        }
        } $($tail)*);
    };
    ($(#[$attrs:meta])* $table_name:ident, $c_val:ident, $decompressed_value:ident, $key:ident
     { $($acc:tt)* } Init { init_size: $init_chunk_size:expr, init_method: Other,
     http_endpoint: $http_endpoint:expr  },
     $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = None;
            const HTTP_ENDPOINT: Option<&'static str> = $http_endpoint;
            const INIT_FLAG: Option<u8> = None;
        }
        } $($tail)*);
    };
    ($(#[$attrs:meta])* $table_name:ident, $c_val:ident, $decompressed_value:ident, $key:ident
     { $($acc:tt)* } CLI { can_insert: False }  $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl IntoTableKey<&str, $key, paste!([<$table_name Data>])> for $table_name {
            fn into_key(value: &str) -> $key {
                let key: $key = value.parse().unwrap();
                println!("decoded key: {key:?}");
                key
            }
            fn into_table_data(_: &str, _: &str) -> paste!([<$table_name Data>]) {
                panic!("inserts not supported for $table_name");
            }
        }
        } $($tail)*);
    };
    ($(#[$attrs:meta])* $table_name:ident,$c_val:ident, $decompressed_value:ident, $key:ident
     { $($acc:tt)* } CLI { can_insert: True }  $($tail:tt)*) => {

        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl IntoTableKey<&str, $key, paste!([<$table_name Data>])> for $table_name {
            fn into_key(value: &str) -> $key {
                let key: $key = value.parse().unwrap();
                println!("decoded key: {key:?}");
                key
            }
            fn into_table_data(key: &str, value: &str) -> paste!([<$table_name Data>]) {
                let key: $key = key.parse().unwrap();
                let value: $decompressed_value = value.parse().unwrap();
                <paste!([<$table_name Data>])>::new(key, value)
            }
        }
        } $($tail)*);
    };

}

compressed_table!(
    Table DexPrice {
        Data {
            #[serde(with = "dex_key")]
            key: DexKey,
            value: DexQuoteWithIndex,
            compressed_value: DexQuoteWithIndexRedefined
        },
        Init {
            init_size: Some(100000),
            init_method: Clickhouse,
            http_endpoint: Some("dex-pricing"),
            init_flag: Some(DEX_PRICE_FLAG)
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table CexPrice {
        Data {
        key: u64,
        value: CexPriceMap,
        compressed_value: CexPriceMapRedefined
        },
        Init {
            init_size: Some(1000),
            init_method: Clickhouse,
            http_endpoint: Some("cex-price"),
            init_flag: Some(CEX_QUOTES_FLAG)
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table CexTrades {
        Data {
        // timestamp in seconds
        key: u64,
        value: CexTradeMap,
        compressed_value: CexTradeMapRedefined
        },
        Init {
            init_size: Some(10000),
            init_method: Clickhouse,
            http_endpoint: None,
            init_flag: Some(CEX_TRADES_FLAG)
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table BlockInfo {
        #[serde_as]
        Data {
            key: u64,
            value: BlockMetadataInner,
            compressed_value: BlockMetadataInnerRedefined
        },
        Init {
            init_size: Some(1000),
            init_method: Clickhouse,
            http_endpoint: Some("block-info"),
                init_flag: Some(META_FLAG)
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table TxTraces {
        #[serde_as]
        Data {
            key: u64,
            #[serde(deserialize_with = "tx_traces_inner::deserialize")]
            value: TxTracesInner,
            compressed_value: TxTracesInnerRedefined
        },
        Init {
            init_size: Some(350),
            init_method: Clickhouse,
            http_endpoint: Some("tx-traces"),
            init_flag: Some(TRACE_FLAG)
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table AddressMeta {
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: AddressMetadata,
            compressed_value: AddressMetadataRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: Some("address-meta"),
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table SearcherEOAs {
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: SearcherInfo,
            compressed_value: SearcherInfoRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: None,
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table SearcherContracts {
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: SearcherInfo,
            compressed_value: SearcherInfoRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: None,
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table Builder {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: BuilderInfo,
            compressed_value: BuilderInfoRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: Some("builder"),
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table AddressToProtocolInfo {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            #[serde(with = "protocol_info")]
            value: ProtocolInfo,
            compressed_value: ProtocolInfoRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: Some("protocol-info"),
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table TokenDecimals {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: TokenInfo
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: Some("token-decimals"),
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table MevBlocks {
        Data {
            key: u64,
            value: MevBlockWithClassified,
            compressed_value: MevBlockWithClassifiedRedefined
        },
        Init {
            init_size: None,
            init_method: Other,
            http_endpoint: None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table PoolCreationBlocks {
        #[serde_as]
        Data {
            key: u64,
            #[serde(with = "pools_libmdbx")]
            value: PoolsToAddresses,
            compressed_value: PoolsToAddressesRedefined
        },
        Init {
            init_size: None,
            init_method: Clickhouse,
            http_endpoint: Some("pool-creation-blocks"),
            init_flag:None
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table InitializedState {
        Data {
            key: u64,
            value: InitializedStateMeta,
            compressed_value: InitializedStateMeta
        },
        Init {
            init_size: None,
            init_method: Other,
            http_endpoint: None
        },
        CLI {
            can_insert: False
        }
    }
);
