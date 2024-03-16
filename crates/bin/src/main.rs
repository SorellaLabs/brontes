use std::error::Error;

use brontes::{
    cli::{
        ext::{InspectorCliExt, NoopInspectorCliExt},
        Args, Commands,
    },
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

    match run::<NoopInspectorCliExt>() {
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

fn run<Ext: InspectorCliExt + clap::Args>() -> eyre::Result<()> {
    let opt = Args::<Ext>::parse();
    match opt.command {
        Commands::Run(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::Database(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::Analytics(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
    }
}

fn init_tracing() {
    let verbosity_level = Level::INFO;
    let directive: Directive = format!("{verbosity_level}").parse().unwrap();

    let layers = vec![brontes_tracing::stdout(directive)];

    brontes_tracing::init(layers);
}
