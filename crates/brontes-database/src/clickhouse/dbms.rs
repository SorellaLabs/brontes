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
    clickhouse_dbms, database_table, remote_clickhouse_table, Database, DatabaseTable,
};
use strum_macros::EnumIter;

use crate::clickhouse::ClickhouseClient;

clickhouse_dbms!(
    BrontesClickhouseTables,
    [
        ClickhouseBundleHeader, //
        ClickhouseMevBlocks,    //
        ClickhouseCexDex,       //
        ClickhouseJit,          //
        ClickhouseJitSandwich,  //
        ClickhouseSandwiches,   //
        ClickhouseAtomicArbs,   // YES
        ClickhouseLiquidations, //
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

remote_clickhouse_table!(BrontesClickhouseTables, "brontes", ClickhouseTxTraces, TxTrace);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseDexPriceMapping,
    DexQuotesWithBlockNumber
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseMevBlocks, MevBlock);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseBundleHeader, BundleHeader);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherInfo,
    JoinedSearcherInfo
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherStats,
    SearcherStatsWithAddress
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseCexDex, CexDex);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseLiquidations, Liquidation);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJitSandwich,
    JitLiquiditySandwich
);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseJit, JitLiquidity);

remote_clickhouse_table!(BrontesClickhouseTables, "mev", ClickhouseSandwiches, Sandwich);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseAtomicArbs,
    AtomicArb,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTokenInfo,
    TokenInfoWithAddress
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderStats,
    BuilderStatsWithAddress
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "ethereum",
    ClickhousePools,
    ProtocolInfoClickhouse
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderInfo,
    BuilderInfoWithAddress
);
