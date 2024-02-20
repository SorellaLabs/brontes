use brontes_types::{
    db::{
        dex::DexQuotes,
        searcher::{JoinedSearcherInfo, SearcherInfo},
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
        ClickhouseDexQuotes,
        ClickhouseTxTraces
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
    ClickhouseDexQuotes,
    DexQuotes,
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

// fix this 1
remote_clickhouse_table!(
    BrontesClickhouseTables,
    "mev",
    ClickhouseCexDex,
    SearcherInfo,
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
