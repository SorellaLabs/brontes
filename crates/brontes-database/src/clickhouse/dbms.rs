use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse,
        builder::{BuilderInfoWithAddress, BuilderStatsWithAddress},
        dex::DexQuotesWithBlockNumber,
        searcher::{JoinedSearcherInfo, SearcherStatsWithAddress},
        token_info::TokenInfoWithAddress,
    },
    mev::*,
    structured_trace::TxTrace,
};
use sorella_db_databases::{
    clickhouse::{
        errors::ClickhouseError,
        tables::{ClickhouseTable, ClickhouseTableType},
    },
    clickhouse_dbms, database_table, remote_clickhouse_table, DatabaseTable,
};
use strum_macros::EnumIter;

use crate::clickhouse::ClickhouseClient;

clickhouse_dbms!(
    BrontesClickhouseTables,
    [
        ClickhouseBundleHeader,
        ClickhouseMevBlocks,
        ClickhouseCexDex,
        ClickhouseJit,
        ClickhouseJitSandwich,
        ClickhouseSandwiches,
        ClickhouseAtomicArbs,
        ClickhouseLiquidations,
        ClickhouseSearcherInfo,
        ClickhouseDexPriceMapping,
        ClickhouseTxTraces,
        ClickhouseTokenInfo,
        ClickhouseSearcherStats,
        ClickhouseBuilderStats,
        ClickhousePools,
        ClickhouseBuilderInfo
    ]
);

remote_clickhouse_table!(BrontesClickhouseTables, "brontes", ClickhouseTxTraces, TxTrace, NO_FILE);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseDexPriceMapping,
    DexQuotesWithBlockNumber,
    NO_FILE
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseMevBlocks, MevBlock, NO_FILE);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseBundleHeader,
    BundleHeader,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherInfo,
    JoinedSearcherInfo,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherStats,
    SearcherStatsWithAddress,
    NO_FILE
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseCexDex, CexDex, NO_FILE);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseLiquidations,
    Liquidation,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJitSandwich,
    JitLiquiditySandwich,
    NO_FILE
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseJit, JitLiquidity, NO_FILE);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseSandwiches, Sandwich, NO_FILE);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseAtomicArbs, AtomicArb, NO_FILE);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTokenInfo,
    TokenInfoWithAddress,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderStats,
    BuilderStatsWithAddress,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "ethereum",
    ClickhousePools,
    ProtocolInfoClickhouse,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderInfo,
    BuilderInfoWithAddress,
    NO_FILE
);
