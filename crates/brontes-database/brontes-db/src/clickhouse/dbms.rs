use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse, builder::BuilderInfoWithAddress,
        dex::DexQuotesWithBlockNumber, normalized_actions::TransactionRoot,
        searcher::JoinedSearcherInfo, token_info::TokenInfoWithAddress,
    },
    mev::*,
    structured_trace::TxTrace,
};
use db_interfaces::{clickhouse_dbms, remote_clickhouse_table};

clickhouse_dbms!(
    BrontesClickhouseTables,
    [
        ClickhouseBundleHeader,
        ClickhouseMevBlocks,
        ClickhouseCexDex,
        ClickhouseSearcherTx,
        ClickhouseJit,
        ClickhouseJitSandwich,
        ClickhouseSandwiches,
        ClickhouseAtomicArbs,
        ClickhouseLiquidations,
        ClickhouseSearcherInfo,
        ClickhouseDexPriceMapping,
        ClickhouseTokenInfo,
        ClickhousePools,
        ClickhouseBuilderInfo,
        ClickhouseTree
    ]
);

impl BrontesClickhouseTables {
    pub const fn is_big(&self) -> bool {
        matches!(
            self,
            BrontesClickhouseTables::ClickhouseTxTraces
                | BrontesClickhouseTables::ClickhouseDexPriceMapping
                | BrontesClickhouseTables::ClickhouseTree
        )
    }
}

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseDexPriceMapping,
    DexQuotesWithBlockNumber,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseMevBlocks,
    MevBlock,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseBundleHeader,
    BundleHeader,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseSearcherTx,
    SearcherTx,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherInfo,
    JoinedSearcherInfo,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseCexDex,
    CexDex,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseLiquidations,
    Liquidation,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJitSandwich,
    JitLiquiditySandwich,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJit,
    JitLiquidity,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseSandwiches,
    Sandwich,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseAtomicArbs,
    AtomicArb,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTokenInfo,
    TokenInfoWithAddress,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "ethereum",
    ClickhousePools,
    ProtocolInfoClickhouse,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderInfo,
    BuilderInfoWithAddress,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTree,
    TransactionRoot,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

pub struct BrontesClickhouseData {
    pub data:         BrontesClickhouseTableDataTypes,
    pub force_insert: bool,
}

macro_rules! db_types {
    ($(($db_type:ident, $db_table:ident)),*) => {
        #[derive(Debug, Clone, serde::Serialize)]
        #[serde(untagged)]
        #[allow(clippy::large_enum_variant)]
        pub enum BrontesClickhouseTableDataTypes {
            $(
                $db_type(Box<$db_type>),
            )*
        }

        paste::paste! {
            impl BrontesClickhouseTableDataTypes {
                pub fn get_db_enum(&self) -> BrontesClickhouseTables {
                    match self {
                        $(
                            BrontesClickhouseTableDataTypes::$db_type(_) =>
                                BrontesClickhouseTables::[<Clickhouse $db_table>],
                        )*
                    }
                }
            }
        }

        $(
            impl From<($db_type, bool)> for BrontesClickhouseData {
                fn from(value: ($db_type, bool)) ->BrontesClickhouseData {
                    BrontesClickhouseData {
                        data: BrontesClickhouseTableDataTypes::$db_type(Box::new(value.0)),
                        force_insert: value.1
                    }
                }
            }

            impl From<$db_type> for BrontesClickhouseTableDataTypes {
                fn from(value: $db_type) -> BrontesClickhouseTableDataTypes {
                    BrontesClickhouseTableDataTypes::$db_type(Box::new(value))
                }
            }

        )*
    };
}

db_types!(
    (DexQuotesWithBlockNumber, DexPriceMapping),
    (MevBlock, MevBlocks),
    (BundleHeader, BundleHeader),
    (SearcherTx, SearcherTx),
    (JoinedSearcherInfo, SearcherInfo),
    (CexDex, CexDex),
    (Liquidation, Liquidations),
    (JitLiquiditySandwich, JitSandwich),
    (JitLiquidity, Jit),
    (Sandwich, Sandwiches),
    (AtomicArb, AtomicArbs),
    (TokenInfoWithAddress, TokenInfo),
    (ProtocolInfoClickhouse, Pools),
    (BuilderInfoWithAddress, BuilderInfo),
    (TransactionRoot, Tree)
);
