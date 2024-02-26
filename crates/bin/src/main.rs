use std::error::Error;

use brontes::{
    cli::{Args, Commands},
    runner,
};
use clap::Parser;
use eyre::eyre;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();
    init_tracing();
    fdlimit::raise_fd_limit().unwrap();

    match run() {
        Ok(()) => {
            info!(target: "brontes", "successful shutdown");
            Ok(())
        }
        Err(e) => {
            error!("Error: {:?}", e);

            let mut source: Option<&dyn Error> = e.source();
            while let Some(err) = source {
                error!("Caused by: {:?}", err);
                source = err.source();
            }
            Err(eyre!("program exited via error"))
        }
    }
}

fn run() -> eyre::Result<()> {
    let opt = Args::parse();
    match opt.command {
        Commands::Run(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::QueryDb(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::AddToDb(command) => runner::run_command_until_exit(|_| command.execute()),
        Commands::TraceRange(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::Analytics(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
    }
}

fn init_tracing() {
    let verbosity_level = Level::INFO;
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();

    let layers = vec![brontes_tracing::stdout(directive)];

    brontes_tracing::init(layers);
}
