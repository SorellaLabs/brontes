use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::Arc,
};

use alloy_primitives::Address;
use async_scoped::{Scope, TokioScope};
use brontes::{Brontes, DataBatching, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::clickhouse::Clickhouse;
use brontes_database_libmdbx::{
    implementation::cursor::LibmdbxCursor,
    tables::{AddressToProtocol, IntoTableKey, Tables},
    Libmdbx,
};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use brontes_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use clap::Parser;
use itertools::Itertools;
use metrics_process::Collector;
use reth_db::{cursor::DbCursorRO, mdbx::RO, table::Table, transaction::DbTx};
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;
mod banner;
mod cli;

use banner::print_banner;
#[cfg(feature = "tests")]
use cli::TraceArg;
use cli::{AddToDb, Args, Commands, DatabaseQuery, DexPricingArgs, Init, RunArgs};

type Inspectors<'a> = [&'a Box<dyn Inspector>; 4];

struct InspectorHolder {
    sandwich: Box<dyn Inspector>,
    cex_dex:  Box<dyn Inspector>,
    jit:      Box<dyn Inspector>,
    backrun:  Box<dyn Inspector>,
}

impl InspectorHolder {
    fn new(quote_token: Address, db: &'static Libmdbx) -> Self {
        Self {
            sandwich: Box::new(SandwichInspector::new(quote_token, db)),
            cex_dex:  Box::new(CexDexInspector::new(quote_token, db)),
            jit:      Box::new(JitInspector::new(quote_token, db)),
            backrun:  Box::new(AtomicBackrunInspector::new(quote_token, db)),
        }
    }

    fn get_inspectors(&'static self) -> Inspectors<'static> {
        [&self.sandwich, &self.cex_dex, &self.jit, &self.backrun]
    }
}

//TODO: Wire in price fetcher + Metadata fetcher

fn main() {
    print_banner();
    dotenv::dotenv().ok();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let directive: Directive = format!("brontes={}", Level::INFO).parse().unwrap();

    let layers = vec![brontes_tracing::stdout(directive)];

    //let subscriber =
    // Registry::default().with(tracing_subscriber::fmt::layer().
    // with_filter(filter));

    //tracing::subscriber::set_global_default(subscriber)
    //  .expect("Could not set global default subscriber");
    brontes_tracing::init(layers);

    match runtime.block_on(run()) {
        Ok(()) => info!(target: "brontes", "SUCCESS!"),
        Err(e) => {
            error!("Error: {:?}", e);

            let mut source: Option<&dyn Error> = e.source();
            while let Some(err) = source {
                error!("Caused by: {:?}", err);
                source = err.source();
            }
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    // initalize_prometheus().await;
    // parse cli
    let opt = Args::parse();

    match opt.command {
        Commands::Run(command) => run_brontes(command).await,
        Commands::Init(command) => init_brontes(command).await,
        Commands::RunBatchWithPricing(command) => run_batch_with_pricing(command).await,
        Commands::QueryDb(command) => query_db(command).await,
        Commands::AddToDb(command) => add_to_db(command).await,
        #[cfg(feature = "tests")]
        Commands::Traces(args) => save_trace(args).await,
    }
}

#[cfg(feature = "tests")]
async fn save_trace(req: TraceArg) -> Result<(), Box<dyn Error>> {
    brontes_core::store_traces_for_block(req.block_num).await;

    Ok(())
}

async fn add_to_db(req: AddToDb) -> Result<(), Box<dyn Error>> {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

    macro_rules! write_to_table {
        ($table:expr, $($tables:ident),+ = $arg0:expr, $arg1:expr) => {
            match $table {
                $(
                    Tables::$tables => {
                        db.write_table(
                            &vec![brontes_database_libmdbx::tables::$tables::into_table_data($arg0, $arg1)]
                            ).unwrap();
                    }
                )+
            }
        };
    }

    write_to_table!(
        req.table,
        CexPrice,
        Metadata,
        DexPrice,
        MevBlocks,
        TokenDecimals,
        AddressToTokens,
        AddressToProtocol,
        SubGraphs,
        PoolCreationBlocks = &req.key,
        &req.value
    );

    Ok(())
}

fn process_range_query<T, E>(
    mut cursor: LibmdbxCursor<T, RO>,
    command: DatabaseQuery,
) -> Result<Vec<T::Value>, Box<dyn Error>>
where
    T: Table,
    T: for<'a> IntoTableKey<&'a str, T::Key, E>,
{
    let range = command.key.split("..").collect_vec();
    let start = range[0];
    let end = range[1];

    let start = T::into_key(start);
    let end = T::into_key(end);

    let mut res = Vec::new();
    for entry in cursor.walk_range(start..end)? {
        if let Ok(entry) = entry {
            res.push(entry.1)
        }
    }

    Ok(res)
}

#[inline(always)]
fn process_single_query<T>(res: Option<T>, _: DatabaseQuery) -> Result<T, Box<dyn Error>> {
    Ok(res.ok_or_else(|| reth_db::DatabaseError::Read(-1))?)
}

async fn query_db(command: DatabaseQuery) -> Result<(), Box<dyn Error>> {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

    let tx = db.ro_tx()?;

    macro_rules! match_table {
        ($table:expr, $fn:expr, $query:ident, $($tables:ident),+ = $args:expr) => {
            match $table {
                $(
                    Tables::$tables => {
                        println!(
                            "{:#?}",
                            $fn(
                                tx.$query::<brontes_database_libmdbx::tables::$tables>(
                                    brontes_database_libmdbx::tables::$tables::into_key($args)
                                    ).unwrap(),
                                command
                            ).unwrap()
                        )
                    }
                )+
            }
        };
        ($table:expr, $fn:expr, $query:ident, $($tables:ident),+) => {
            match $table {
                $(
                    Tables::$tables => {
                        println!(
                            "{:#?}",
                            $fn(
                                tx.$query::<brontes_database_libmdbx::tables::$tables>()?,
                                command
                            )?
                        )
                    }
                )+
            }
        };
    }

    if command.key.contains("..") {
        match_table!(
            command.table,
            process_range_query,
            new_cursor,
            CexPrice,
            Metadata,
            DexPrice,
            MevBlocks,
            TokenDecimals,
            AddressToTokens,
            AddressToProtocol,
            PoolCreationBlocks,
            SubGraphs
        );
    } else {
        match_table!(
            command.table,
            process_single_query,
            get,
            CexPrice,
            Metadata,
            DexPrice,
            MevBlocks,
            TokenDecimals,
            AddressToTokens,
            AddressToProtocol,
            SubGraphs,
            PoolCreationBlocks = &command.key
        );
    }

    Ok(())
}

async fn run_brontes(run_config: RunArgs) -> Result<(), Box<dyn Error>> {
    initialize_prometheus().await;

    // Fetch required environment variables.
    let db_path = get_env_vars()?;

    let max_tasks = determine_max_tasks(run_config.max_tasks);

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener = PoirotMetricsListener::new(metrics_rx);
    tokio::spawn(metrics_listener);

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx =
        Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;
    let clickhouse = Clickhouse::default();

    let inspector_holder = Box::leak(Box::new(InspectorHolder::new(
        run_config.quote_asset.parse().unwrap(),
        &libmdbx,
    )));

    let inspectors: Inspectors = inspector_holder.get_inspectors();

    let (manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);
    tokio::spawn(manager);

    let parser = DParser::new(
        metrics_tx,
        &libmdbx,
        tracer,
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let classifier = Classifier::new(&libmdbx, tx.clone());

    #[cfg(not(feature = "local"))]
    let chain_tip = parser.get_latest_block_number().unwrap();
    #[cfg(feature = "local")]
    let chain_tip = parser.get_latest_block_number().await.unwrap();

    let brontes = Brontes::new(
        run_config.start_block,
        run_config.end_block,
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

async fn init_brontes(init_config: Init) -> Result<(), Box<dyn Error>> {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

    let clickhouse = Arc::new(Clickhouse::default());

    let libmdbx = Arc::new(Libmdbx::init_db(brontes_db_endpoint, None)?);
    if init_config.init_libmdbx {
        // currently inits all tables
        let range =
            if let (Some(start), Some(end)) = (init_config.start_block, init_config.end_block) {
                Some((start, end))
            } else {
                None
            };
        libmdbx
            .clear_and_initialize_tables(
                clickhouse.clone(),
                init_config
                    .tables_to_init
                    .unwrap_or({
                        if init_config.download_dex_pricing {
                            let tables = Tables::ALL.to_vec();
                            //tables.retain(|table| table != &Tables::CexPrice);
                            //println!("TABLES: {:?}", tables);
                            tables
                        } else {
                            Tables::ALL_NO_DEX.to_vec()
                        }
                    })
                    .as_slice(),
                range,
            )
            .await?;
    }

    //TODO: Joe, have it download the full range of metadata from the MEV DB so
    // they can run everything in parallel
    Ok(())
}

async fn run_batch_with_pricing(config: DexPricingArgs) -> Result<(), Box<dyn Error>> {
    assert!(config.start_block <= config.end_block);
    info!(?config);

    let db_path = get_env_vars()?;

    let max_tasks = determine_max_tasks(config.max_tasks);

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener = PoirotMetricsListener::new(metrics_rx);
    tokio::spawn(metrics_listener);

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx =
        Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;

    let inspector_holder =
        Box::leak(Box::new(InspectorHolder::new(config.quote_asset.parse().unwrap(), &libmdbx)));
    let inspectors: Inspectors = inspector_holder.get_inspectors();

    let (manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);
    tokio::spawn(manager);

    let parser = DParser::new(
        metrics_tx,
        &libmdbx,
        tracer,
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let cpus = determine_max_tasks(config.max_tasks);

    let range = config.end_block - config.start_block;

    let cpus_min = range / config.min_batch_size;

    let mut scope: TokioScope<'_, ()> = unsafe { Scope::create() };

    // the amount of cpu's we want to use
    let cpus = std::cmp::min(cpus_min, cpus);
    let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

    for (i, mut chunk) in (config.start_block..=config.end_block)
        .chunks(chunk_size.try_into().unwrap())
        .into_iter()
        .enumerate()
    {
        let start_block = chunk.next().unwrap();
        let end_block = chunk.last().unwrap_or(start_block);

        info!(batch_id = i, start_block, end_block, "starting batch");

        scope.spawn(spawn_batches(
            config.quote_asset.parse().unwrap(),
            0,
            i as u64,
            start_block,
            end_block,
            &parser,
            libmdbx,
            &inspectors,
        ));
    }

    // collect and wait
    scope.collect().await;
    info!("finnished running all batch , shutting down");
    drop(scope);
    std::thread::spawn(move || {
        drop(parser);
    });

    Ok(())
}

async fn spawn_batches(
    quote_asset: Address,
    run_id: u64,
    batch_id: u64,
    start_block: u64,
    end_block: u64,
    parser: &DParser<'_, TracingClient>,
    libmdbx: &'static Libmdbx,
    inspectors: &Inspectors<'_>,
) {
    DataBatching::new(
        quote_asset,
        run_id,
        batch_id,
        start_block,
        end_block,
        &parser,
        &libmdbx,
        &inspectors,
    )
    .await
}

fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks as u64,
        None => {
            let cpus = num_cpus::get_physical();
            (cpus as f64 * 0.5) as u64 // 50% of physical cores
        }
    }
}

async fn initialize_prometheus() {
    // initializes the prometheus endpoint
    initialize(
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::from(PROMETHEUS_ENDPOINT_IP)),
            PROMETHEUS_ENDPOINT_PORT,
        ),
        Collector::default(),
    )
    .await
    .unwrap();
    info!("Initialized prometheus endpoint");
}

fn get_env_vars() -> Result<String, Box<dyn Error>> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}

/*
fn get_reth_provider<T>() -> Result<Provider<T>, Box<dyn Error>> {
    let reth_url = env::var("RETH_ENDPOINT").expect("No RETH_DB Endpoint in .env");
    let reth_port = env::var("RETH_PORT").expect("No DB port.env");
    let url = format!("{reth_url}:{reth_port}");
    Provider::new(&url).unwrap()
}
 */
