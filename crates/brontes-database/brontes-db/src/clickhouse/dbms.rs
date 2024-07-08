use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse, block_analysis::BlockAnalysis,
        builder::BuilderInfoWithAddress, dex::DexQuotesWithBlockNumber,
        normalized_actions::TransactionRoot, searcher::JoinedSearcherInfo,
        token_info::TokenInfoWithAddress,
    },
    mev::*,
};
use db_interfaces::{clickhouse_dbms, remote_clickhouse_table};

clickhouse_dbms!(
    BrontesClickhouseTables,
    "eth_cluster0",
    [
        BrontesDex_Price_Mapping,
        BrontesBlock_Analysis,
        MevMev_Blocks,
        MevBundle_Header,
        MevSearcher_Tx,
        BrontesSearcher_Info,
        MevCex_Dex,
        MevLiquidations,
        MevJit_Sandwich,
        MevJit,
        MevSandwiches,
        MevAtomic_Arbs,
        BrontesToken_Info,
        EthereumPools,
        BrontesBuilder_Info,
        BrontesTree
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
    BlockAnalysis,
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
    [Brontes, Searcher_Info],
    JoinedSearcherInfo,
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
    [Brontes, Builder_Info],
    BuilderInfoWithAddress,
    "crates/brontes-database/brontes-db/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    [Brontes, Tree],
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
                                BrontesClickhouseTables::$db_table,
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
    (DexQuotesWithBlockNumber, BrontesDex_Price_Mapping),
    (MevBlock, MevMev_Blocks),
    (BundleHeader, MevBundle_Header),
    (SearcherTx, MevSearcher_Tx),
    (JoinedSearcherInfo, BrontesSearcher_Info),
    (CexDex, MevCex_Dex),
    (Liquidation, MevLiquidations),
    (JitLiquiditySandwich, MevJit_Sandwich),
    (JitLiquidity, MevJit),
    (Sandwich, MevSandwiches),
    (AtomicArb, MevAtomic_Arbs),
    (TokenInfoWithAddress, BrontesToken_Info),
    (ProtocolInfoClickhouse, EthereumPools),
    (BuilderInfoWithAddress, BrontesBuilder_Info),
    (TransactionRoot, BrontesTree),
    (BlockAnalysis, BrontesBlock_Analysis)
);
