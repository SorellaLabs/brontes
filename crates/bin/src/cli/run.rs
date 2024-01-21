use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use alloy_primitives::Address;
use async_scoped::{Scope, TokioScope};
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{
        cursor::CompressedCursor,
        tables::{AddressToProtocol, CompressedTable, IntoTableKey, Tables},
        Libmdbx,
    },
};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use brontes_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use clap::Parser;
use itertools::Itertools;
use metrics_process::Collector;
use reth_db::mdbx::RO;
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

use super::{determine_max_tasks, get_env_vars, init_all_inspectors};
use crate::{
    runner::CliContext, Brontes, DataBatching, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT,
};

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:   Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    pub max_tasks:   Option<u64>,
    /// Optional quote asset, if omitted it will default to USDC
    #[arg(long, short, default_value = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")]
    pub quote_asset: String,
}
impl RunArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        // Fetch required environment variables.
        let db_path = get_env_vars()?;
        let quote_asset = self.quote_asset.parse()?;
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);

        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx =
            Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;
        let clickhouse = Clickhouse::default();

        let inspectors = init_all_inspectors(quote_asset, libmdbx);

        let tracer = TracingClient::new(Path::new(&db_path), max_tasks, task_executor.clone());

        let parser = DParser::new(
            metrics_tx,
            &libmdbx,
            tracer.clone(),
            Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
        );

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let classifier = Classifier::new(&libmdbx, tx.clone(), tracer.into());

        #[cfg(not(feature = "local"))]
        let chain_tip = parser.get_latest_block_number().unwrap();
        #[cfg(feature = "local")]
        let chain_tip = parser.get_latest_block_number().await.unwrap();

        let brontes = Brontes::new(
            self.start_block,
            self.end_block,
            chain_tip,
            max_tasks.into(),
            &parser,
            &clickhouse,
            &libmdbx,
            &classifier,
            &inspectors,
        );
        brontes.await;

        info!("finnished running brontes, shutting down");
        Ok(())
    }
}
