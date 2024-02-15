use brontes_types::{
    db::{dex::DexQuotes, searcher::SearcherInfo},
    mev::{BundleHeader, JitLiquidity, JitLiquiditySandwich, Liquidation, MevBlock},
    structured_trace::TxTraces,
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
    "ethereum",
    ClickhouseTxTraces,
    TxTraces,
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
    "mev",
    ClickhouseSearcherInfo,
    SearcherInfo,
    NO_FILE
);

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
