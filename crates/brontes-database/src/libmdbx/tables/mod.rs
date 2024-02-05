use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use brontes_pricing::{Protocol, SubGraphsEntry};
use brontes_types::{
    db::{
        address_to_tokens::{PoolTokens, PoolTokensRedefined},
        cex::{CexPriceMap, CexPriceMapRedefined},
        dex::{DexKey, DexQuoteWithIndex, DexQuoteWithIndexRedefined},
        metadata::{BlockMetadataInner, BlockMetadataInnerRedefined},
        mev_block::{MevBlockWithClassified, MevBlockWithClassifiedRedefined},
        pool_creation_block::{PoolsToAddresses, PoolsToAddressesRedefined},
        token_info::TokenInfo,
        traces::{TxTracesInner, TxTracesInnerRedefined},
    },
    pair::Pair,
    price_graph_types::SubGraphsEntryRedefined,
    serde_primitives::*,
    traits::TracingProvider,
};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use crate::libmdbx::{types::ReturnKV, LibmdbxData};

mod const_sql;
use alloy_primitives::Address;
use const_sql::*;
use paste::paste;
use reth_db::{table::Table, TableType};

use super::{
    initialize::LibmdbxInitializer, types::IntoTableKey, utils::static_bindings, CompressedTable,
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
        clear_table: bool,
    ) -> eyre::Result<()> {
        match self {
            Tables::TokenDecimals => {
                initializer
                    .clickhouse_init_no_args::<TokenDecimals, TokenDecimalsData>(clear_table)
                    .await
            }
            Tables::AddressToTokens => {
                initializer
                    .clickhouse_init_no_args::<AddressToTokens, AddressToTokensData>(clear_table)
                    .await
            }
            Tables::AddressToProtocol => {
                initializer
                    .clickhouse_init_no_args::<AddressToProtocol, AddressToProtocolData>(
                        clear_table,
                    )
                    .await
            }
            Tables::CexPrice => {
                initializer
                    .initialize_table_from_clickhouse::<CexPrice, CexPriceData>(
                        block_range,
                        clear_table,
                    )
                    .await
            }
            Tables::BlockInfo => {
                initializer
                    .initialize_table_from_clickhouse::<BlockInfo, BlockInfoData>(
                        block_range,
                        clear_table,
                    )
                    .await
            }
            Tables::PoolCreationBlocks => {
                initializer
                    .initialize_table_from_clickhouse::<PoolCreationBlocks, PoolCreationBlocksData>(
                        block_range,
                        clear_table,
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
    BlockInfo,
    DexPrice,
    PoolCreationBlocks,
    MevBlocks,
    SubGraphs,
    TxTraces
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
            const NAME: &'static str = stringify!($table_name);
            type Key = $key;
            type Value = $c_val;
        }

        impl std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", stringify!($table_name))
            }
        }

        #[cfg(test)]
        #[allow(unused)]
        impl $table_name {
            pub(crate) async fn test_initialized_data(
                clickhouse: &crate::libmdbx::Clickhouse,
                libmdbx: &crate::libmdbx::Libmdbx,
                block_range: Option<(u64, u64)>
            ) -> eyre::Result<(usize, usize)> {
                paste::paste!{
                    crate::libmdbx::test_utils::compare_clickhouse_libmdbx_data::<$table_name, [<$table_name Data>]>(clickhouse, libmdbx, block_range).await
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
            compressed_value: PoolTokensRedefined
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
        compressed_value: CexPriceMapRedefined
        },
        Init {
            init_size: Some(10_000),
            init_method: Clickhouse
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
            init_size: Some(50_000),
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
            compressed_value: DexQuoteWithIndexRedefined
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
            compressed_value: PoolsToAddressesRedefined
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
            compressed_value: MevBlockWithClassifiedRedefined
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
            compressed_value: SubGraphsEntryRedefined
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
            compressed_value: TxTracesInnerRedefined
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
