use std::error::Error;

use brontes::{
    banner,
    cli::{Args, Commands},
    runner,
};
use clap::Parser;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

fn main() {
    banner::print_banner();
    dotenv::dotenv().ok();
    init_tracing();

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
        Commands::RangeWithDexPrice(command) => {
            runner::run_command_until_exit(|ctx| command.execute(ctx))
        }
        Commands::Init(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::QueryDb(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::AddToDb(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::TraceRange(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
    }
}

fn init_tracing() {
    // all lower level logging directives include higher level ones (Trace includes
    // all, Debug includes all but Trace, ...)
    let verbosity_level = Level::INFO; // Error >= Warn >= Info >= Debug >= Trace
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();
    let layers = vec![brontes_tracing::stdout(directive)];

    brontes_tracing::init(layers);
}
