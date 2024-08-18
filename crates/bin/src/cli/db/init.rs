use std::{path::Path, sync::Arc};

use brontes_database::{libmdbx::LibmdbxInit, Tables};
use brontes_types::{db::cex::CexExchange, init_thread_pools};
use clap::Parser;
use indicatif::MultiProgress;
use itertools::Itertools;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_clickhouse, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Init {
    /// Initialize the local Libmdbx DB
    #[arg(long, short)]
    pub init_libmdbx:              bool,
    /// Libmdbx tables to initialize:
    ///     TokenDecimals
    ///     AddressToTokens
    ///     AddressToProtocol
    ///     CexPrice
    ///     Metadata
    ///     PoolState
    ///     DexPrice
    ///     CexTrades
    #[arg(long, short, requires = "init_libmdbx", value_delimiter = ',')]
    pub tables_to_init:            Option<Vec<Tables>>,
    /// The sliding time window (BEFORE) for cex quotes relative to the block
    /// time
    #[arg(long = "price-tw-before", default_value_t = 3)]
    pub quotes_time_window_before: u64,
    /// The sliding time window (AFTER) for cex quotes relative to the block
    /// time
    #[arg(long = "price-tw-after", default_value_t = 3)]
    pub quotes_time_window_after:  u64,
    /// The sliding time window (BEFORE) for cex trades relative to the block
    /// number
    #[arg(long = "trades-tw-before", default_value_t = 3)]
    pub trades_time_window_before: u64,
    /// The sliding time window (AFTER) for cex trades relative to the block
    /// number
    #[arg(long = "trades-tw-after", default_value_t = 3)]
    pub trades_time_window_after:  u64,
    /// Centralized exchanges that the cex-dex inspector will consider
    #[arg(
        long,
        short,
        default_value = "Binance,Coinbase,Okex,BybitSpot,Kucoin",
        value_delimiter = ','
    )]
    pub cex_exchanges:             Vec<CexExchange>,
    /// Start Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub start_block:               Option<u64>,
    /// End Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub end_block:                 Option<u64>,
    /// Download Dex Prices from Sorella's MEV DB for the given block range. If
    /// false it will run the dex pricing locally using raw on-chain data
    #[arg(long, short, default_value = "false")]
    pub download_dex_pricing:      bool,
}

impl Init {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        init_thread_pools(10);
        let task_executor = ctx.task_executor;

        let libmdbx =
            static_object(load_database(&task_executor, brontes_db_path, None, None).await?);
        let clickhouse = static_object(load_clickhouse(Default::default(), None).await?);

        let tracer = Arc::new(get_tracing_provider(Path::new(&db_path), 10, task_executor.clone()));

        if self.init_libmdbx {
            // currently inits all tables
            let range = if let (Some(start), Some(end)) = (self.start_block, self.end_block) {
                Some((start, end))
            } else {
                None
            };

            task_executor
                .spawn_critical("init", async move {
                    let tables = Tables::ALL.to_vec();

                    let multi = MultiProgress::default();
                    let tables_with_progress = Arc::new(
                        tables
                            .clone()
                            .into_iter()
                            .map(|table| {
                                (table, table.build_init_state_progress_bar(&multi, 1000000000))
                            })
                            .collect_vec(),
                    );

                    futures::future::join_all(
                        self.tables_to_init
                            .unwrap_or(tables)
                            .into_iter()
                            .map(|table| {
                                let tracer = tracer.clone();
                                let tables_with_progress = tables_with_progress.clone();
                                async move {
                                    libmdbx
                                        .initialize_tables(
                                            clickhouse,
                                            tracer,
                                            table,
                                            false,
                                            range,
                                            tables_with_progress,
                                        )
                                        .await
                                        .unwrap();
                                }
                            }),
                    )
                    .await;
                })
                .await
                .unwrap();
        }

        Ok(())
    }
}
