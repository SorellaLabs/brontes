use futures::{pin_mut, Future};
use tokio::{runtime::Runtime, task::JoinHandle};

/// executes tasks on the runtime
/// used for a thread pool for the simulator
pub(crate) struct Executor {
    pub runtime: Runtime,
}

impl Executor {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        Self { runtime }
    }

    /// Spawns a task with a result output depending on the given [TaskKind]
    pub fn spawn_result_task_as<F, R>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send,
    {
        let task = async move {
            pin_mut!(fut);
            fut.await
        };

        let handle = self.runtime.handle().clone();
        match task_kind {
            TaskKind::Default => handle.spawn(task),
            TaskKind::Blocking => self.runtime.spawn_blocking(move || handle.block_on(fut)),
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
        let handle = self.runtime.handle().clone();
        match task_kind {
            TaskKind::Default => handle.spawn(fut),
            TaskKind::Blocking => self.runtime.spawn_blocking(move || handle.block_on(fut)),
        }
    }

    /// Spawns a future blocking tokio runtime
    pub fn block_on_rt<F>(&self, fut: F) -> ()
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.block_on(fut)
    }
}

/// specifies a blocking or non blocking task
pub(crate) enum TaskKind {
    Default,
    Blocking,
}
