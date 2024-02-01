use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use brontes_pricing::{Protocol, SubGraphsEntry};
use brontes_types::{
    db::{
        address_to_tokens::PoolTokens, cex::CexPriceMap, dex::DexQuoteWithIndex,
        metadata::MetadataInner, mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses, token_info::TokenInfo, traces::TxTracesInner,
    },
    pair::Pair,
    serde_primitives::*,
    traits::TracingProvider,
};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use crate::libmdbx::{
    types::{utils::static_bindings, ReturnKV},
    LibmdbxData,
};

mod const_sql;
use alloy_primitives::Address;
use const_sql::*;
use paste::paste;
use reth_db::{table::Table, TableType};

use super::{
    initialize::LibmdbxInitializer,
    types::{
        address_to_tokens::{ArchivedLibmdbxPoolTokens, LibmdbxPoolTokens},
        cex_price::{ArchivedLibmdbxCexPriceMap, LibmdbxCexPriceMap},
        dex_price::{ArchivedLibmdbxDexQuoteWithIndex, DexKey, LibmdbxDexQuoteWithIndex},
        metadata::{ArchivedLibmdbxMetadataInner, LibmdbxMetadataInner},
        mev_block::{ArchivedLibmdbxMevBlockWithClassified, LibmdbxMevBlockWithClassified},
        pool_creation_block::{ArchivedLibmdbxPoolsToAddresses, LibmdbxPoolsToAddresses},
        subgraphs::{ArchivedLibmdbxSubGraphsEntry, LibmdbxSubGraphsEntry},
        traces::{ArchivedLibmdbxTxTracesInner, LibmdbxTxTracesInner},
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
                    .clickhouse_init_no_args::<TokenDecimals, TokenDecimalsData>()
                    .await
            }
            Tables::AddressToTokens => {
                initializer
                    .clickhouse_init_no_args::<AddressToTokens, AddressToTokensData>()
                    .await
            }
            Tables::AddressToProtocol => {
                initializer
                    .clickhouse_init_no_args::<AddressToProtocol, AddressToProtocolData>()
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
                    rkyv::check_archived_root::<Self>(&buf[..]).unwrap();


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
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $c_val;
        }

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
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
        implement_table_value_codecs_with_zc!($c_val);
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
        #[derive(Debug, Clone, Row, serde::Serialize, serde::Deserialize)]
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
     { $($acc:tt)* } Init { init_size: $init_chunk_size:expr, init_method: Clickhouse },
     $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = Some(paste! {[<$table_name InitQuery>]});
        }
        } $($tail)*);
    };
    ($(#[$attrs:meta])* $table_name:ident, $c_val:ident, $decompressed_value:ident, $key:ident
     { $($acc:tt)* } Init { init_size: $init_chunk_size:expr, init_method: Other },
     $($tail:tt)*) => {
        compressed_table!($(#[$attrs])* $table_name, $c_val, $decompressed_value, $key {
            $($acc)*
        impl CompressedTable for $table_name {
            type DecompressedValue = $decompressed_value;
            const INIT_CHUNK_SIZE: Option<usize> = $init_chunk_size;
            const INIT_QUERY: Option<&'static str> = None;
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
    Table TokenDecimals {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            value: TokenInfo
        },
        Init {
            init_size: None,
            init_method: Clickhouse
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table AddressToTokens {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            #[serde(with = "pool_tokens")]
            value: PoolTokens,
            compressed_value: LibmdbxPoolTokens
        },
        Init {
            init_size: None,
            init_method: Clickhouse
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table AddressToProtocol {
        #[serde_as]
        Data {
            #[serde(with = "address_string")]
            key: Address,
            #[serde(with = "static_bindings")]
            value: Protocol
        },
        Init {
            init_size: None,
            init_method: Clickhouse
        },
        CLI {
            can_insert: True
        }
    }
);

compressed_table!(
    Table CexPrice {
        Data {
        key: u64,
        value: CexPriceMap,
        compressed_value: LibmdbxCexPriceMap
        },
        Init {
            init_size: Some(200000),
            init_method: Clickhouse
        },
        CLI {
            can_insert: False
        }
    }

);

compressed_table!(
    Table Metadata {
        #[serde_as]
        Data {
            key: u64,
            value: MetadataInner,
            compressed_value: LibmdbxMetadataInner
        },
        Init {
            init_size: Some(200000),
            init_method: Clickhouse
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table DexPrice {
        Data {
            key: DexKey,
            value: DexQuoteWithIndex,
            compressed_value: LibmdbxDexQuoteWithIndex
        },
        Init {
            init_size: None,
            init_method: Other
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
            compressed_value: LibmdbxPoolsToAddresses
        },
        Init {
            init_size: None,
            init_method: Clickhouse
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
            compressed_value: LibmdbxMevBlockWithClassified
        },
        Init {
            init_size: None,
            init_method: Other
        },
        CLI {
            can_insert: False
        }
    }
);

compressed_table!(
    Table SubGraphs {
        Data {
            key: Pair,
            value: SubGraphsEntry,
            compressed_value: LibmdbxSubGraphsEntry
        },
        Init {
            init_size: None,
            init_method: Other
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
            value: TxTracesInner,
            compressed_value: LibmdbxTxTracesInner
        },
        Init {
            init_size: None,
            init_method: Other
        },
        CLI {
            can_insert: False
        }
    }
);
