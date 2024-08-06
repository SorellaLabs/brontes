use std::{env, path::Path};

use ahash::HashSetExt;
use alloy_primitives::Address;
use brontes_core::LibmdbxReader;
use brontes_database::{
    clickhouse::cex_config::CexDownloadConfig, libmdbx::initialize::LibmdbxInitializer,
};
use brontes_types::{
    constants::USDT_ADDRESS,
    db::cex::{trades::CexTrades, CexExchange},
    init_threadpools,
    pair::Pair,
    FastHashMap, FastHashSet,
};
use clap::Parser;
use clickhouse::Row;
use db_interfaces::{
    clickhouse::{
        client::ClickhouseClient,
        config::ClickhouseConfig,
        dbms::{ClickhouseDBMS, NullDBMS},
    },
    errors::DatabaseError,
    Database,
};
use eyre::Result;
use prettytable::{Cell, Row, Table};
use reth_tracing_ext::TracingClient;
use serde::{Deserialize, Serialize};

use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};

const SECONDS_TO_US: u64 = 1_000_000;

/// downloads a range of data from clickhouse
#[derive(Debug, Parser)]
pub struct ClickhouseDownload {
    /// start block
    #[arg(long, short)]
    pub start_block: u64,
    /// end block
    #[arg(long, short)]
    pub end_block:   u64,
    /// table to download
    #[arg(short, long)]
    pub table:       brontes_database::Tables,
}

impl ClickhouseDownload {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let task_executor = ctx.task_executor;

        let cex_config = CexDownloadConfig::default();
        let libmdbx = static_object(load_libmdbx(&task_executor, brontes_db_endpoint)?);
        let clickhouse: ClickhouseClient<NullDBMS> = get_clickhouse_env();

        let initializer = LibmdbxInitializer::new(
            libmdbx,
            &clickhouse,
            Arc::new(TracingClient::new(&Path::new(&brontes_db_endpoint), 10, task_executor)),
        );

        let pre = std::time::Instant::now();
        initializer
            .initialize(self.table, Some((self.start_block, self.end_block)), Arc::new(vec![]))
            .await;

        let time_taken = std::time::Instant::now().duration_since(pre);
        println!("Table: {:?} -- Time Elapsed {}", self.table, time_taken.as_secs());

        Ok(())
    }
}
