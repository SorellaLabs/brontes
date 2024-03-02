use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse,
        builder::{BuilderInfoWithAddress, BuilderStatsWithAddress},
        dex::DexQuotesWithBlockNumber,
        searcher,
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

use crate::clickhouse::ClickhouseClient;

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
        ClickhouseTxTraces,
        ClickhouseTokenInfo,
        ClickhouseSearcherStats,
        ClickhouseBuilderStats,
        ClickhousePools,
        ClickhouseBuilderInfo
    ]
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTxTraces,
    TxTrace,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseDexPriceMapping,
    DexQuotesWithBlockNumber,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseMevBlocks,
    MevBlock,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseBundleHeader,
    BundleHeader,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseSearcherTx,
    SearcherTx,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherInfo,
    JoinedSearcherInfo,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseSearcherStats,
    SearcherStatsWithAddress,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseCexDex,
    CexDex,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseLiquidations,
    Liquidation,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJitSandwich,
    JitLiquiditySandwich,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJit,
    JitLiquidity,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseSandwiches,
    Sandwich,
    "crates/brontes-database/src/clickhouse/tables/"
);

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
    TokenInfoWithAddress,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderStats,
    BuilderStatsWithAddress,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "ethereum",
    ClickhousePools,
    ProtocolInfoClickhouse,
    "crates/brontes-database/src/clickhouse/tables/"
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseBuilderInfo,
    BuilderInfoWithAddress,
    "crates/brontes-database/src/clickhouse/tables/"
);
