use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use brontes_pricing::{Protocol, SubGraphsEntry};
use brontes_types::{
    db::{
        address_to_tokens::PoolTokens, cex::CexPriceMap, dex::DexQuoteWithIndex,
        metadata::MetadataInner, mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses, traces::TxTracesInner,
    },
    pair::Pair,
    traits::TracingProvider,
};

mod const_sql;
use alloy_primitives::Address;
use const_sql::*;
use paste::paste;
use reth_db::{table::Table, TableType};

use super::{
    initialize::LibmdbxInitializer,
    types::{
        address_to_protocol::AddressToProtocolData,
        address_to_tokens::{AddressToTokensData, ArchivedLibmdbxPoolTokens, LibmdbxPoolTokens},
        cex_price::{ArchivedLibmdbxCexPriceMap, CexPriceData, LibmdbxCexPriceMap},
        dex_price::{
            ArchivedLibmdbxDexQuoteWithIndex, DexKey, DexPriceData, LibmdbxDexQuoteWithIndex,
        },
        metadata::{ArchivedLibmdbxMetadataInner, LibmdbxMetadataInner, MetadataData},
        mev_block::{
            ArchivedLibmdbxMevBlockWithClassified, LibmdbxMevBlockWithClassified, MevBlocksData,
        },
        pool_creation_block::{
            ArchivedLibmdbxPoolsToAddresses, LibmdbxPoolsToAddresses, PoolCreationBlocksData,
        },
        subgraphs::{ArchivedLibmdbxSubGraphsEntry, LibmdbxSubGraphsEntry, SubGraphsData},
        token_decimals::TokenDecimalsData,
        traces::{ArchivedLibmdbxTxTracesInner, LibmdbxTxTracesInner, TxTracesData},
        IntoTableKey,
    },
    CompressedTable,
};

pub const NUM_TABLES: usize = 10;

macro_rules! tables {
    ($($table:ident),*) => {
        #[derive(Debug, PartialEq, Copy, Clone)]
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
    pub(crate) async fn initialize_table<T: TracingProvider>(
        &self,
        initializer: &LibmdbxInitializer<T>,
        block_range: Option<(u64, u64)>,
    ) -> eyre::Result<()> {
        match self {
            Tables::TokenDecimals => {
                initializer
                    .initialize_table_from_clickhouse_no_args::<TokenDecimals, TokenDecimalsData>(
                    )
                    .await
            }
            Tables::AddressToTokens => {
                initializer
                    .initialize_table_from_clickhouse_no_args::<AddressToTokens, AddressToTokensData>(
                    )
                    .await
            }
            Tables::AddressToProtocol => {
                initializer
                    .initialize_table_from_clickhouse_no_args::<AddressToProtocol, AddressToProtocolData>(
                    )
                    .await
            }
            Tables::CexPrice => {
                initializer
                    .initialize_table_from_clickhouse::<CexPrice, CexPriceData>(block_range)
                    .await
            }
            Tables::Metadata => {
                initializer
                    .initialize_table_from_clickhouse::<Metadata, MetadataData>(block_range)
                    .await
            }
            Tables::PoolCreationBlocks => {
                initializer
                    .initialize_table_from_clickhouse::<PoolCreationBlocks, PoolCreationBlocksData>(
                        block_range,
                    )
                    .await
            }
            Tables::DexPrice => Ok(()),
            Tables::MevBlocks => Ok(()),
            Tables::SubGraphs => Ok(()),
            Tables::TxTraces => Ok(()),
        }
    }
}

tables!(
    TokenDecimals,
    AddressToTokens,
    AddressToProtocol,
    CexPrice,
    Metadata,
    DexPrice,
    PoolCreationBlocks,
    MevBlocks,
    SubGraphs,
    TxTraces
);

#[macro_export]
macro_rules! implement_table_value_codecs_with_zc {
    ($table_value:ident) => {
        impl alloy_rlp::Encodable for $table_value {
            fn encode(&self, out: &mut dyn bytes::BufMut) {
                let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

                out.put_slice(&encoded)
            }
        }

        impl alloy_rlp::Decodable for $table_value {
            fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
                let archived: &paste!([<Archived $table_value>]) =
                    unsafe { rkyv::archived_root::<Self>(buf) };

                let this = rkyv::Deserialize::deserialize(archived, &mut rkyv::Infallible).unwrap();

                Ok(this)
            }
        }

        impl reth_db::table::Compress for $table_value {
            type Compressed = Vec<u8>;

            fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
                let mut encoded = Vec::new();
                alloy_rlp::Encodable::encode(&self, &mut encoded);
                let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

                buf.put_slice(&encoded_compressed);
            }
        }

        impl reth_db::table::Decompress for $table_value {
            fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
                let binding = value.as_ref().to_vec();

                let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
                let buf = &mut encoded_decompressed.as_slice();

                alloy_rlp::Decodable::decode(buf).map_err(|_| reth_db::DatabaseError::Decode)
            }
        }
    };
}

#[macro_export]
macro_rules! compressed_table {
    // WITH $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ident | $init_method:tt
    | $init_chunk_size:expr, $decompressed_value:ident = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value | $init_method
        | Some($init_chunk_size), $decompressed_value = $($table)*);
        implement_table_value_codecs_with_zc!($compressed_value);
    };

    // WITH $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ident
    | $init_method:tt | $decompressed_value:ident = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $compressed_value
        | $init_method | None, $decompressed_value = $($table)*);
        implement_table_value_codecs_with_zc!($compressed_value);
    };

    // WITHOUT $compressed_value
    // WITH $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $init_method:tt
    | $init_chunk_size:expr, $decompressed_value:ident = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $decompressed_value
        | $init_method | Some($init_chunk_size), $decompressed_value = $($table)*);
    };

    // WITHOUT $compressed_value
    // WITHOUT $init_chunk_size
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $init_method:tt
    | $decompressed_value:ident = $($table:tt)*) => {
        table!($(#[$docs])+ ( $table_name ) $key | $decompressed_value
        | $init_method | None, $decompressed_value = $($table)*);
    };
}

macro_rules! table {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty | $compressed_value:ident
    | $init_method:tt | $init_chunk_size:expr, $decompressed_value:ident = $($table:tt)*) => {
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

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
            }
        }

        table!($table_name, $decompressed_value, $init_chunk_size, $init_method);
    };
    ($table_name:ident, $decompressed_value:ident, $init_chunk_size:expr, Clickhouse) => {
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = Some(paste! {[<$table_name InitQuery>]});
        }
    };
    ($table_name:ident, $decompressed_value:ident, $init_chunk_size:expr, Other) => {
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = None;
        }
    };
    ($table_name:ident, $key:ty, $value:ident, True) => {
        fn into_table_data(key: &str, value: &str) -> paste!([<$table_name Data>]) {
            let key: $key = key.parse().unwrap();
            let value: $value = value.parse().unwrap();
            <paste!([<$table_name Data>])>::new(key, value)
        }

    };
    ($table_name:ident, $key:ty, $value:ident, False) => {
        fn into_table_data(_: &str, _: &str) -> paste!([<$table_name Data>]) {
            panic!("inserts not supported for $table_name");
        }
    }
}

compressed_table!(
    /// token address -> decimals
    ( TokenDecimals ) Address | Clickhouse | u8 = False
);

compressed_table!(
    /// Address -> tokens in pool
    ( AddressToTokens ) Address | LibmdbxPoolTokens | Clickhouse | PoolTokens = False
);

compressed_table!(
    /// Address -> Static protocol enum
    ( AddressToProtocol ) Address | Clickhouse | Protocol = True
);

compressed_table!(
    /// block num -> cex prices
    ( CexPrice ) u64 | LibmdbxCexPriceMap | Clickhouse | 200000,  CexPriceMap = False
);

compressed_table!(
    /// block num -> metadata
    ( Metadata ) u64 | LibmdbxMetadataInner | Clickhouse | 200000,  MetadataInner = False
);

compressed_table!(
    /// block number concat tx idx -> cex quotes
    ( DexPrice ) DexKey | LibmdbxDexQuoteWithIndex | Other | DexQuoteWithIndex = False
);

compressed_table!(
    /// block number -> pools created in block
    ( PoolCreationBlocks ) u64 | LibmdbxPoolsToAddresses | Clickhouse | PoolsToAddresses = False
);

compressed_table!(
    /// block number -> mev block with classified mev
    ( MevBlocks ) u64 | LibmdbxMevBlockWithClassified | Other | MevBlockWithClassified = False
);

compressed_table!(
    /// pair -> Vec<(block_number, entry)>
    ( SubGraphs ) Pair | LibmdbxSubGraphsEntry | Other | SubGraphsEntry = False
);

compressed_table!(
    /// block number -> tx traces
    ( TxTraces ) u64 | LibmdbxTxTracesInner | Other | TxTracesInner = False
);
