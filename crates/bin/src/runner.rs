use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use brontes_metrics::prometheus_exporter::initialize;
use futures::pin_mut;
use metrics_process::Collector;
use reth_tasks::{TaskExecutor, TaskManager};
use tracing::{error, info, trace};

use crate::{PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};

pub fn run_command_until_exit<F, E>(command: impl FnOnce(CliContext) -> F) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
    E: Send + Sync + From<std::io::Error> + From<reth_tasks::PanickedTaskError> + 'static,
{
    let AsyncCliRunner { context, task_manager, tokio_runtime } = AsyncCliRunner::new()?;
    // initalize prometheus if we don't already have a endpoint
    tokio_runtime.block_on(try_initialize_prometheus());

    // Executes the command until it finished or ctrl-c was fired
    let task_manager = tokio_runtime
        .block_on(run_to_completion_or_panic(task_manager, run_until_ctrl_c(command(context))))?;

    // after the command has finished or exit signal was received we shutdown the
    // task manager which fires the shutdown signal to all tasks spawned via the
    // task executor and awaiting on tasks spawned with graceful shutdown
    task_manager.graceful_shutdown_with_timeout(std::time::Duration::from_secs(10));

    // drop the tokio runtime on a separate thread because drop blocks until its
    // pools (including blocking pool) are shutdown. In other words
    // `drop(tokio_runtime)` would block the current thread but we want to exit
    // right away.
    println!("DROPPING ON OTHER THREAD");
    std::thread::spawn(move || drop(tokio_runtime));
    Ok(())
}

/// Creates a new default tokio multi-thread [Runtime](tokio::runtime::Runtime)
/// with all features enabled
pub fn tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
}

/// tries to start prometheus. will fail if prometheus is
/// already running
async fn try_initialize_prometheus() {
    // initializes the prometheus endpoint
    if let Err(e) = initialize(
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::from(PROMETHEUS_ENDPOINT_IP)),
            PROMETHEUS_ENDPOINT_PORT,
        ),
        Collector::default(),
    )
    .await
    {
        error!(error=%e, "failed to initialize prometheus");
    } else {
        info!("Initialized prometheus endpoint");
    }
}

async fn run_to_completion_or_panic<F, E>(mut tasks: TaskManager, fut: F) -> Result<TaskManager, E>
where
    F: Future<Output = Result<(), E>>,
    E: Send + Sync + From<reth_tasks::PanickedTaskError> + 'static,
{
    {
        pin_mut!(fut);
        tokio::select! {
            err = &mut tasks => {
                return Err(err.into())
            },
            res = fut => res?,
        }
    }
    Ok(tasks)
}
pub async fn run_until_ctrl_c<F, E>(fut: F) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
    E: Send + Sync + 'static + From<std::io::Error>,
{
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let sigterm = stream.recv();
        pin_mut!(sigterm, ctrl_c, fut);

        tokio::select! {
            _ = ctrl_c => {
                trace!(target: "reth::cli",  "Received ctrl-c");
            },
            _ = sigterm => {
                trace!(target: "reth::cli",  "Received SIGTERM");
            },
            res = fut => res?,
        }
    }

    #[cfg(not(unix))]
    {
        pin_mut!(ctrl_c, fut);

        tokio::select! {
            _ = ctrl_c => {
                trace!(target: "reth::cli",  "Received ctrl-c");
            },
            res = fut => res?,
        }
    }

    Ok(())
}

struct AsyncCliRunner {
    context:       CliContext,
    task_manager:  TaskManager,
    tokio_runtime: tokio::runtime::Runtime,
}

// === impl AsyncCliRunner ===

impl AsyncCliRunner {
    /// Attempts to create a tokio Runtime and additional context required to
    /// execute commands asynchronously.
    fn new() -> Result<Self, std::io::Error> {
        let tokio_runtime = tokio_runtime()?;
        let task_manager = TaskManager::new(tokio_runtime.handle().clone());
        let task_executor = task_manager.executor();
        Ok(Self { context: CliContext { task_executor }, task_manager, tokio_runtime })
    }
}

/// Additional context provided by the [CliRunner] when executing commands
pub struct CliContext {
    /// Used to execute/spawn tasks
    pub task_executor: TaskExecutor,
}
