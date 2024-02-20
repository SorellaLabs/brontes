use brontes_types::{
    db::{
        builder::BuilderStatsWithAddress,
        dex::DexQuotesWithBlockNumber,
        searcher::{JoinedSearcherInfo, SearcherStatsWithAddress},
        token_info::TokenInfoWithAddress,
    },
    mev::*,
    structured_trace::TxTrace,
};
use sorella_db_databases::{
    clickhouse::{
        dbms::ClickhouseDBMS,
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
        ClickhouseLiquidations,
        ClickhouseSearcherInfo,
        ClickhouseDexPriceMapping,
        ClickhouseTxTraces,
        ClickhouseTokenInfo,
        ClickhouseSearcherStats,
        ClickhouseBuilderStats
    ]
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseTxTraces,
    TxTrace,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "brontes",
    ClickhouseDexPriceMapping,
    DexQuotesWithBlockNumber,
    NO_FILE
);

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseMevBlocks,
    MevBlock,
    NO_FILE
);

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

// fix this 1
remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseCexDex,
    CexDex,
    NO_FILE
);

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

remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseJit,
    JitLiquidity,
    NO_FILE
);

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
