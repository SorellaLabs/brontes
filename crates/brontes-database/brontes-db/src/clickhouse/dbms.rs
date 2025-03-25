use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse, block_analysis::BlockAnalysis,
        dex::DexQuotesWithBlockNumber, normalized_actions::TransactionRoot,
        token_info::TokenInfoWithAddress, DbDataWithRunId, RunId,
    },
    mev::*,
};
use db_interfaces::{clickhouse_dbms, remote_clickhouse_table};

clickhouse_dbms!(
    BrontesClickhouseTables,
    [
        BrontesDex_Price_Mapping,
        BrontesBlock_Analysis,
        MevMev_Blocks,
        MevBundle_Header,
        MevSearcher_Tx,
        MevCex_Dex_Quotes,
        MevCex_Dex,
        MevLiquidations,
        MevJit_Sandwich,
        MevJit,
        MevSandwiches,
        MevAtomic_Arbs,
        BrontesToken_Info,
        EthereumPools,
        BrontesTree,
        BrontesRun_Id
    ]
);

impl BrontesClickhouseTables {
    pub const fn is_big(&self) -> bool {
        matches!(
            self,
            BrontesClickhouseTables::BrontesDex_Price_Mapping
                | BrontesClickhouseTables::BrontesTree
        )
    }
}

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Dex_Price_Mapping],
    DexQuotesWithBlockNumber,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Block_Analysis],
    DbDataWithRunId<BlockAnalysis>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Mev_Blocks],
    DbDataWithRunId<MevBlock>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Bundle_Header],
    DbDataWithRunId<BundleHeader>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Searcher_Tx],
    DbDataWithRunId<SearcherTx>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Cex_Dex],
    DbDataWithRunId<CexDex>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Cex_Dex_Quotes],
    DbDataWithRunId<CexDexQuote>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Liquidations],
    DbDataWithRunId<Liquidation>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Jit_Sandwich],
    DbDataWithRunId<JitLiquiditySandwich>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Jit],
    DbDataWithRunId<JitLiquidity>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Sandwiches],
    DbDataWithRunId<Sandwich>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Mev, Atomic_Arbs],
    DbDataWithRunId<AtomicArb>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Token_Info],
    TokenInfoWithAddress,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Ethereum, Pools],
    ProtocolInfoClickhouse,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Tree],
    DbDataWithRunId<TransactionRoot>,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Run_Id],
    RunId,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

pub struct BrontesClickhouseData {
    pub data:         BrontesClickhouseTableDataTypes,
    pub force_insert: bool,
}

macro_rules! db_types {
    ($(($db_type:ident, $db_table:ident, $t:tt)),*) => {
        db_types!(enum_s {}, $($db_type, $t,)*);

        paste::paste! {
            impl BrontesClickhouseTableDataTypes {
                pub fn get_db_enum(&self) -> BrontesClickhouseTables {
                    match self {
                        $(
                            BrontesClickhouseTableDataTypes::$db_type(_) =>
                                BrontesClickhouseTables::$db_table,
                        )*
                    }
                }
            }
        }

        $(
            db_types!($db_type, $t);

        )*
    };
    ($db_type:ident, true) => {
            impl From<($db_type, bool, u64)> for BrontesClickhouseData {
                fn from(value: ($db_type, bool, u64)) ->BrontesClickhouseData {
                    BrontesClickhouseData {
                        data: BrontesClickhouseTableDataTypes::$db_type(Box::new(
                                      DbDataWithRunId {
                                          table: value.0,
                                          run_id: value.2
                                      }
                                      )),
                        force_insert: value.1
                    }
                }
            }

    };
    ($db_type:ident, false) => {
        impl From<($db_type, bool)> for BrontesClickhouseData {
            fn from(value: ($db_type, bool)) ->BrontesClickhouseData {
                BrontesClickhouseData {
                    data: BrontesClickhouseTableDataTypes::$db_type(Box::new(value.0)),
                    force_insert: value.1
                }
            }
        }
    };
    (enum_s  {$($acc:tt)* }, $db_type:ident, true, $($tail:tt)*) => {
        db_types!(enum_s {
            $($acc)*
            $db_type(Box<DbDataWithRunId<$db_type>>),
        }, $($tail)*);
    };
    (enum_s {$($acc:tt)* }, $db_type:ident, false, $($tail:tt)*) => {
        db_types!(enum_s {
            $($acc)*
            $db_type(Box<$db_type>),
        }, $($tail)*);
    };
    (enum_s {$($acc:tt)*},$(,)*) => {
        #[derive(Debug, Clone, serde::Serialize)]
        #[serde(untagged)]
        #[allow(clippy::large_enum_variant)]
        pub enum BrontesClickhouseTableDataTypes {
            $($acc)*
        }
    }
}

db_types!(
    (DexQuotesWithBlockNumber, BrontesDex_Price_Mapping, false),
    (MevBlock, MevMev_Blocks, true),
    (BundleHeader, MevBundle_Header, true),
    (SearcherTx, MevSearcher_Tx, true),
    (CexDex, MevCex_Dex, true),
    (CexDexQuote, MevCex_Dex_Quotes, true),
    (Liquidation, MevLiquidations, true),
    (JitLiquiditySandwich, MevJit_Sandwich, true),
    (JitLiquidity, MevJit, true),
    (Sandwich, MevSandwiches, true),
    (AtomicArb, MevAtomic_Arbs, true),
    (TokenInfoWithAddress, BrontesToken_Info, false),
    (ProtocolInfoClickhouse, EthereumPools, false),
    (TransactionRoot, BrontesTree, true),
    (BlockAnalysis, BrontesBlock_Analysis, true),
    (RunId, BrontesRun_Id, false)
);
