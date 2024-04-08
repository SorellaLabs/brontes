use std::error::Error;

use brontes::{
    cli::{Args, Commands},
    runner,
};
use clap::Parser;
use eyre::eyre;
use tracing::{error, info};

fn main() -> eyre::Result<()> {
    dotenv::dotenv().expect("Failed to load .env file");
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
        Commands::Database(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::Analytics(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
    }
}

fn init_tracing() {
    let default_level = "info";

    let layers = vec![brontes_tracing::stdout(default_level)];

    brontes_tracing::init(layers);
}
