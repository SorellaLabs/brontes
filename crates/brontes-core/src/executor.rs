use core::panic;

use futures::{pin_mut, Future};
use tokio::task::JoinHandle;

/// executes tasks on the runtime
/// used for a thread pool for the simulator
pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    /// Spawns a task with a result output depending on the given [TaskKind]
    pub fn spawn_result_task_as<F, R>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let task = async move {
            pin_mut!(fut);
            fut.await
        };

        let handle = tokio::runtime::Handle::current();
        match task_kind {
            TaskKind::Default => handle.spawn(task),
            TaskKind::Blocking => panic!(),
        }
    }

    /// Spawns a task depending on the given [TaskKind]
    pub fn spawn_task_as<F>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = async move {
            pin_mut!(fut);
            let _ = fut.await;
        };

        self.spawn_on_rt(task, task_kind)
    }

    /// Spawns a future on the tokio runtime depending on the [TaskKind]
    fn spawn_on_rt<F>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::runtime::Handle::current();
        match task_kind {
            TaskKind::Default => handle.spawn(fut),
            TaskKind::Blocking => panic!(),
        }
    }
}

/// specifies a blocking or non blocking task
pub enum TaskKind {
    Default,
    Blocking,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
