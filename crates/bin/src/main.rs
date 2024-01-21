use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

#[cfg(feature = "tests")]
use brontes::cli::TraceArg;
use brontes::{
    banner,
    cli::{Args, Commands},
    runner, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT,
};
use brontes_metrics::prometheus_exporter::initialize;
use clap::Parser;
use metrics_process::Collector;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

fn main() {
    banner::print_banner();
    dotenv::dotenv().ok();

    let brontes_directive: Directive = format!("brontes={}", Level::INFO).parse().unwrap();
    let tracing_directive: Directive = format!("reth-tracing-ext={}", Level::INFO).parse().unwrap();

    let layers = vec![
        brontes_tracing::stdout(tracing_directive),
        brontes_tracing::stdout(brontes_directive),
    ];

    //let subscriber =
    // Registry::default().with(tracing_subscriber::fmt::layer().
    // with_filter(filter));

    //tracing::subscriber::set_global_default(subscriber)
    //  .expect("Could not set global default subscriber");
    brontes_tracing::init(layers);

    match run() {
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

fn run() -> eyre::Result<()> {
    let opt = Args::parse();
    match opt.command {
        Commands::Run(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::RunBatchWithPricing(command) => {
            runner::run_command_until_exit(|ctx| command.execute(ctx))
        }
        Commands::Init(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::QueryDb(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::AddToDb(command) => runner::run_command_until_exit(|_| command.execute()),
        #[cfg(feature = "tests")]
        Commands::Traces(args) => runner::run_command_until_exit(|_| command.execute()),
    }
}

#[allow(unused)]
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
