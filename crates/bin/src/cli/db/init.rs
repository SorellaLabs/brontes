use std::{env, path::Path, sync::Arc};

use brontes_database::{clickhouse::cex_config::CexDownloadConfig, libmdbx::LibmdbxInit, Tables};
use brontes_types::{db::cex::CexExchange, init_threadpools};
use clap::Parser;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_clickhouse, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Init {
    /// Initialize the local Libmdbx DB
    #[arg(long, short)]
    pub init_libmdbx:                  bool,
    /// Libmdbx tables to init:
    ///     TokenDecimals
    ///     AddressToTokens
    ///     AddressToProtocol
    ///     CexPrice
    ///     Metadata
    ///     PoolState
    ///     DexPrice
    ///     CexTrades
    #[arg(long, short, requires = "init_libmdbx", value_delimiter = ',')]
    pub tables_to_init:                Option<Vec<Tables>>,
    /// The sliding time window (BEFORE) for cex prices relative to the block
    /// timestamp
    #[arg(long = "price-tw-before", default_value = "12")]
    pub cex_price_time_window_before:  u64,
    /// The sliding time window (AFTER) for cex prices relative to the block
    /// timestamp
    #[arg(long = "price-tw-after", default_value = "0")]
    pub cex_price_time_window_after:   u64,
    /// The sliding time window (BEFORE) for cex trades relative to the block
    /// timestamp
    #[arg(long = "trades-tw-before", default_value = "6")]
    pub cex_trades_time_window_before: u64,
    /// The sliding time window (AFTER) for cex trades relative to the block
    /// timestamp
    #[arg(long = "trades-tw-after", default_value = "6")]
    pub cex_trades_time_window_after:  u64,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(
        long,
        short,
        default_value = "Binance,Coinbase,Okex,BybitSpot,Kucoin",
        value_delimiter = ','
    )]
    pub cex_exchanges:                 Vec<CexExchange>,
    /// Start Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub start_block:                   Option<u64>,
    /// End Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub end_block:                     Option<u64>,
    /// Download Dex Prices from Sorella's MEV DB for the given block range. If
    /// false it will run the dex pricing locally using raw on-chain data
    #[arg(long, short, default_value = "false")]
    pub download_dex_pricing:          bool,
}

impl Init {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

        init_threadpools(10);
        let task_executor = ctx.task_executor;

        let cex_download_config = CexDownloadConfig::new(
            (self.cex_price_time_window_before, self.cex_price_time_window_after),
            (self.cex_trades_time_window_before, self.cex_trades_time_window_after),
            self.cex_exchanges,
        );
        let libmdbx = static_object(load_database(brontes_db_endpoint)?);
        let clickhouse = static_object(load_clickhouse(cex_download_config).await?);

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
                    libmdbx
                        .initialize_tables(
                            clickhouse,
                            tracer,
                            self.tables_to_init
                                .unwrap_or({
                                    if self.download_dex_pricing {
                                        //TODO: Joe add non dex price download behaviour
                                        Tables::ALL.to_vec()
                                    } else {
                                        Tables::ALL.to_vec()
                                    }
                                })
                                .as_slice(),
                            false,
                            range,
                        )
                        .await
                        .unwrap();
                })
                .await
                .unwrap();
        }

        Ok(())
    }
}
