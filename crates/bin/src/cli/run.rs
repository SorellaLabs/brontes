use std::{env, error::Error};

use brontes_database::libmdbx::{
    cursor::CompressedCursor,
    tables::{AddressToProtocol, CompressedTable, IntoTableKey, Tables},
    Libmdbx,
};
use clap::Parser;
use reth_db::mdbx::RO;

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
    pub async fn execute(self) -> Result<(), Box<dyn Error>> {
        initialize_prometheus().await;

        // Fetch required environment variables.
        let db_path = get_env_vars()?;

        let max_tasks = determine_max_tasks(self.max_tasks);

        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        tokio::spawn(metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx =
            Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;
        let clickhouse = Clickhouse::default();

        let inspector_holder =
            Box::leak(Box::new(InspectorHolder::new(self.quote_asset.parse().unwrap(), &libmdbx)));

        let inspectors: Inspectors = inspector_holder.get_inspectors();

        let (manager, tracer) =
            TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);
        tokio::spawn(manager);

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
        std::thread::spawn(move || {
            drop(parser);
        });

        Ok(())
    }
}
