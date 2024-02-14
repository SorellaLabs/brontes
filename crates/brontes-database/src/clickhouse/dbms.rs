use brontes_types::structured_trace::TxTraces;
use sorella_db_databases::{
    clickhouse::{
        dbms::ClickhouseDBMS,
        errors::ClickhouseError,
        tables,
        tables::{ClickhouseTable, ClickhouseTableType},
    },
    clickhouse_dbms, database_table, remote_clickhouse_table,
    tables::DatabaseTable,
};
use strum_macros::EnumIter;

use crate::clickhouse::ClickhouseClient;

clickhouse_dbms!(
    BrontesClickhouseTables,
    [
        // BundleHeader,
        // MevBlocks,
        // CexDex,
        // Jit,
        // JitSandwich,
        // Liquidations,
        // Sandwich,
        TxTrace
    ]
);

remote_clickhouse_table!(BrontesClickhouseTables, TxTrace, TxTraces, NO_FILE);
