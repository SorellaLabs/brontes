use std::{env, path::Path, sync::Arc};

use brontes_database::{libmdbx::LibmdbxInit, Tables};
use brontes_types::init_threadpools;
use clap::Parser;
use indicatif::MultiProgress;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_clickhouse, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Init {
    /// Initialize the local Libmdbx DB
    #[arg(long, short)]
    pub init_libmdbx:         bool,
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
    pub tables_to_init:       Option<Vec<Tables>>,
    /// Start Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub start_block:          Option<u64>,
    /// End Block to download metadata from Sorella's MEV DB
    #[arg(long, short)]
    pub end_block:            Option<u64>,
    /// Download Dex Prices from Sorella's MEV DB for the given block range. If
    /// false it will run the dex pricing locally using raw on-chain data
    #[arg(long, short, default_value = "false")]
    pub download_dex_pricing: bool,
}

impl Init {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

        init_threadpools(10);
        let task_executor = ctx.task_executor;

        let libmdbx = static_object(load_database(brontes_db_endpoint)?);
        let clickhouse = static_object(load_clickhouse().await?);

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
                    let bar = MultiProgress::default();
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
                            bar,
                            0
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
