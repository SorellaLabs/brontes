//! Ported from Reth tasks for our own use cases
use std::{
    any::Any,
    fmt::{Display, Formatter},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
};

use futures::{
    future::{select, BoxFuture, FusedFuture, Shared},
    pin_mut, Future, FutureExt, TryFutureExt,
};
use reth_tasks::{shutdown::GracefulShutdown, TaskSpawner, TaskSpawnerExt};
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        oneshot, OnceCell,
    },
    task::JoinHandle,
};
use tracing::{debug, error, Instrument};

static EXECUTOR: OnceCell<BrontesTaskExecutor> = OnceCell::const_new();

#[derive(Debug)]
#[must_use = "BrontesTaskManager must be polled to monitor critical tasks"]
pub struct BrontesTaskManager {
    /// Handle to the tokio runtime this task manager is associated with.
    ///
    /// See [`Handle`] docs.
    handle:            Handle,
    /// Sender half for sending panic signals to this type
    panicked_tasks_tx: UnboundedSender<PanickedTaskError>,
    /// Listens for panicked tasks
    panicked_tasks_rx: UnboundedReceiver<PanickedTaskError>,
    /// The [Signal] to fire when all tasks should be shutdown.
    ///
    /// This is fired when dropped.
    signal:            Option<Signal>,
    /// Receiver of the shutdown signal.
    on_shutdown:       Shutdown,
    /// How many [GracefulShutdown] tasks are currently active
    graceful_tasks:    Arc<AtomicUsize>,
}

impl BrontesTaskManager {
    /// Returns a a `BrontesTaskManager` over the currently running Runtime.
    ///
    /// # Panics
    ///
    /// This will panic if called outside the context of a Tokio runtime.
    pub fn current() -> Self {
        let handle = Handle::current();
        Self::new(handle, false)
    }

    /// Create a new instance connected to the given handle's tokio runtime.
    pub fn new(handle: Handle, no_panic_override: bool) -> Self {
        let (panicked_tasks_tx, panicked_tasks_rx) = unbounded_channel();
        let (signal, on_shutdown) = signal();

        let tx = panicked_tasks_tx.clone();

        let bt_level = std::env::var("RUST_BACKTRACE").unwrap_or(String::from("0"));

        if bt_level == "0" && !no_panic_override {
            std::panic::set_hook(Box::new(move |info| {
                let location = info.location().unwrap();

                let msg = match info.payload().downcast_ref::<&'static str>() {
                    Some(s) => *s,
                    None => match info.payload().downcast_ref::<String>() {
                        Some(s) => &s[..],
                        None => "Box<dyn Any>",
                    },
                };
                let error_msg = format!("panic happened at {location}:\n {msg}");

                let _ = tx.send(PanickedTaskError::new("thread", Box::new(error_msg)));
            }));
        }

        let this = Self {
            handle,
            panicked_tasks_tx,
            panicked_tasks_rx,
            signal: Some(signal),
            on_shutdown,
            graceful_tasks: Arc::new(AtomicUsize::new(0)),
        };

        let _ = EXECUTOR.set(this.executor());

        this
    }

    /// Returns a new `TaskExecutor` that can spawn new tasks onto the tokio
    /// runtime this type is connected to.
    pub fn executor(&self) -> BrontesTaskExecutor {
        BrontesTaskExecutor {
            handle:            self.handle.clone(),
            on_shutdown:       self.on_shutdown.clone(),
            panicked_tasks_tx: self.panicked_tasks_tx.clone(),
            graceful_tasks:    Arc::clone(&self.graceful_tasks),
        }
    }

    /// Fires the shutdown signal and awaits until all tasks are shutdown.
    pub fn graceful_shutdown(self) {
        let _ = self.do_graceful_shutdown(None);
    }

    /// Fires the shutdown signal and awaits until all tasks are shutdown.
    ///
    /// Returns true if all tasks were shutdown before the timeout elapsed.
    pub fn graceful_shutdown_with_timeout(self, timeout: std::time::Duration) -> bool {
        self.do_graceful_shutdown(Some(timeout))
    }

    fn do_graceful_shutdown(self, timeout: Option<std::time::Duration>) -> bool {
        drop(self.signal);
        let when = timeout.map(|t| std::time::Instant::now() + t);
        while self.graceful_tasks.load(Ordering::Relaxed) > 0 {
            if when
                .map(|when| std::time::Instant::now() > when)
                .unwrap_or(false)
            {
                debug!("graceful shutdown timed out");
                return false
            }
            std::hint::spin_loop();
        }

        debug!("gracefully shut down");
        true
    }
}

impl Future for BrontesTaskManager {
    type Output = PanickedTaskError;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let err = ready!(self.get_mut().panicked_tasks_rx.poll_recv(cx));
        Poll::Ready(err.expect("stream can not end"))
    }
}

#[derive(Debug, Clone)]
pub struct BrontesTaskExecutor {
    /// Handle to the tokio runtime this task manager is associated with.
    ///
    /// See [`Handle`] docs.
    handle:            Handle,
    /// Receiver of the shutdown signal.
    on_shutdown:       Shutdown,
    /// Sender half for sending panic signals to this type
    panicked_tasks_tx: UnboundedSender<PanickedTaskError>,
    /// How many [GracefulShutdown] tasks are currently active
    graceful_tasks:    Arc<AtomicUsize>,
}

impl BrontesTaskExecutor {
    /// panics if not  in a task_manager scope
    pub fn current() -> &'static Self {
        EXECUTOR
            .get()
            .expect("not running in a brontes task manager scope")
    }

    /// Causes a shutdown to occur.
    pub fn trigger_shutdown(&self, task_name: &'static str) {
        let _ = self
            .panicked_tasks_tx
            .send(PanickedTaskError { error: None, task_name });
    }

    /// Returns the [Handle] to the tokio runtime.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Returns the receiver of the shutdown signal.
    pub fn on_shutdown_signal(&self) -> &Shutdown {
        &self.on_shutdown
    }

    /// Runs a future to completion on this Handle's associated Runtime.
    #[track_caller]
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }

    /// Spawns a future on the tokio runtime depending on the [TaskKind]
    fn spawn_on_rt<F>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        match task_kind {
            TaskKind::Default => self.handle.spawn(fut),
            TaskKind::Blocking => {
                let handle = self.handle.clone();
                self.handle.spawn_blocking(move || handle.block_on(fut))
            }
        }
    }

    /// Spawns a regular task depending on the given [TaskKind]
    fn spawn_task_as<F>(&self, fut: F, task_kind: TaskKind) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let on_shutdown = self.on_shutdown.clone();

        // Wrap the original future to increment the finished tasks counter upon
        // completion
        let task = {
            async move {
                pin_mut!(fut);
                let _ = select(on_shutdown, fut).await;
            }
        }
        .in_current_span();

        self.spawn_on_rt(task, task_kind)
    }

    /// Spawns the task onto the runtime.
    /// The given future resolves as soon as the [Shutdown] signal is received.
    ///
    /// See also [`Handle::spawn`].
    pub fn spawn<F>(&self, fut: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_task_as(fut, TaskKind::Default)
    }

    /// Spawns a blocking task onto the runtime.
    /// The given future resolves as soon as the [Shutdown] signal is received.
    ///
    /// See also [`Handle::spawn_blocking`].
    pub fn spawn_blocking<F>(&self, fut: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_task_as(fut, TaskKind::Blocking)
    }

    /// Spawns the task onto the runtime.
    /// The given future resolves as soon as the [Shutdown] signal is received.
    ///
    /// See also [`Handle::spawn`].
    pub fn spawn_with_signal<F>(&self, f: impl FnOnce(Shutdown) -> F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let on_shutdown = self.on_shutdown.clone();
        let fut = f(on_shutdown);

        let task = fut.in_current_span();

        self.handle.spawn(task)
    }

    /// Spawns a critical task depending on the given [TaskKind]
    fn spawn_critical_as<F>(
        &self,
        name: &'static str,
        fut: F,
        task_kind: TaskKind,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let on_shutdown = self.on_shutdown.clone();

        // wrap the task in catch unwind
        let task = std::panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!("{task_error}");
                let _ = panicked_tasks_tx.send(task_error);
            })
            .in_current_span();

        let task = async move {
            pin_mut!(task);
            let _ = select(on_shutdown, task).await;
        };

        self.spawn_on_rt(task, task_kind)
    }

    /// This spawns a critical blocking task onto the runtime.
    /// The given future resolves as soon as the [Shutdown] signal is received.
    ///
    /// If this task panics, the `TaskManager` is notified.
    pub fn spawn_critical_blocking<F>(&self, name: &'static str, fut: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_critical_as(name, fut, TaskKind::Blocking)
    }

    /// This spawns a critical task onto the runtime.
    /// The given future resolves as soon as the [Shutdown] signal is received.
    ///
    /// If this task panics, the `TaskManager` is notified.
    pub fn spawn_critical<F>(&self, name: &'static str, fut: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_critical_as(name, fut, TaskKind::Default)
    }

    /// This spawns a critical task onto the runtime.
    ///
    /// If this task panics, the `TaskManager` is notified.
    pub fn spawn_critical_with_shutdown_signal<F>(
        &self,
        name: &'static str,
        f: impl FnOnce(Shutdown) -> F,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let on_shutdown = self.on_shutdown.clone();
        let fut = f(on_shutdown);

        // wrap the task in catch unwind
        let task = std::panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!("{task_error}");
                let _ = panicked_tasks_tx.send(task_error);
            })
            .map(|_| ())
            .in_current_span();

        self.handle.spawn(task)
    }

    pub fn get_graceful_shutdown(&self) -> GracefulShutdown {
        let on_shutdown = LocalGracefulShutdown::new(
            self.on_shutdown.clone(),
            LocalGracefulShutdownGuard::new(Arc::clone(&self.graceful_tasks)),
        );
        unsafe { std::mem::transmute(on_shutdown) }
    }

    /// This spawns a critical task onto the runtime.
    ///
    /// If this task panics, the TaskManager is notified.
    /// The TaskManager will wait until the given future has completed before
    /// shutting down.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn t(executor: reth_tasks::TaskExecutor) {
    ///
    /// executor.spawn_critical_with_graceful_shutdown_signal("grace", |shutdown| async move {
    ///     // await the shutdown signal
    ///     let guard = shutdown.await;
    ///     // do work before exiting the program
    ///     tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    ///     // allow graceful shutdown
    ///     drop(guard);
    /// });
    /// # }
    /// ```
    pub fn spawn_critical_with_graceful_shutdown_signal<F>(
        &self,
        name: &'static str,
        f: impl FnOnce(GracefulShutdown) -> F,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let on_shutdown = LocalGracefulShutdown::new(
            self.on_shutdown.clone(),
            LocalGracefulShutdownGuard::new(Arc::clone(&self.graceful_tasks)),
        );

        #[allow(clippy::missing_transmute_annotations)]
        let fut = f(unsafe { std::mem::transmute(on_shutdown) });

        // wrap the task in catch unwind
        let task = std::panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!("{task_error}");
                let _ = panicked_tasks_tx.send(task_error);
            })
            .map(|_| ())
            .in_current_span();

        self.handle.spawn(task)
    }

    /// This spawns a regular task onto the runtime.
    ///
    /// The [BrontesTaskManager] will wait until the given future has completed
    /// before shutting down.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn t(executor: reth_tasks::TaskExecutor) {
    ///
    /// executor.spawn_with_graceful_shutdown_signal(|shutdown| async move {
    ///     // await the shutdown signal
    ///     let guard = shutdown.await;
    ///     // do work before exiting the program
    ///     tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    ///     // allow graceful shutdown
    ///     drop(guard);
    /// });
    /// # }
    /// ```
    pub fn spawn_with_graceful_shutdown_signal<F>(
        &self,
        f: impl FnOnce(GracefulShutdown) -> F,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let on_shutdown = LocalGracefulShutdown::new(
            self.on_shutdown.clone(),
            LocalGracefulShutdownGuard::new(Arc::clone(&self.graceful_tasks)),
        );
        #[allow(clippy::missing_transmute_annotations)]
        let fut = f(unsafe { std::mem::transmute(on_shutdown) });

        self.handle.spawn(fut)
    }
}

impl TaskSpawner for BrontesTaskExecutor {
    fn spawn(&self, fut: BoxFuture<'static, ()>) -> JoinHandle<()> {
        self.spawn(fut)
    }

    fn spawn_critical(&self, name: &'static str, fut: BoxFuture<'static, ()>) -> JoinHandle<()> {
        BrontesTaskExecutor::spawn_critical(self, name, fut)
    }

    fn spawn_blocking(&self, fut: BoxFuture<'static, ()>) -> JoinHandle<()> {
        self.spawn_blocking(fut)
    }

    fn spawn_critical_blocking(
        &self,
        name: &'static str,
        fut: BoxFuture<'static, ()>,
    ) -> JoinHandle<()> {
        BrontesTaskExecutor::spawn_critical_blocking(self, name, fut)
    }
}

impl TaskSpawnerExt for BrontesTaskExecutor {
    fn spawn_critical_with_graceful_shutdown_signal<F>(
        &self,
        name: &'static str,
        f: impl FnOnce(GracefulShutdown) -> F,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        BrontesTaskExecutor::spawn_critical_with_graceful_shutdown_signal(self, name, f)
    }

    fn spawn_with_graceful_shutdown_signal<F>(
        &self,
        f: impl FnOnce(GracefulShutdown) -> F,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        BrontesTaskExecutor::spawn_with_graceful_shutdown_signal(self, f)
    }
}

/// A Future that resolves when the shutdown event has been fired.
#[derive(Debug, Clone)]
pub struct Shutdown(Shared<oneshot::Receiver<()>>);

impl Future for Shutdown {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();
        if pin.0.is_terminated() || pin.0.poll_unpin(cx).is_ready() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Shutdown signal that fires either manually or on drop by closing the channel
#[derive(Debug)]
pub struct Signal(oneshot::Sender<()>);

impl Signal {
    /// Fire the signal manually.
    pub fn fire(self) {
        let _ = self.0.send(());
    }
}

/// Create a channel pair that's used to propagate shutdown event
pub fn signal() -> (Signal, Shutdown) {
    let (sender, receiver) = oneshot::channel();
    (Signal(sender), Shutdown(receiver.shared()))
}

#[derive(Debug, thiserror::Error)]
pub struct PanickedTaskError {
    task_name: &'static str,
    error:     Option<String>,
}

impl Display for PanickedTaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let task_name = self.task_name;
        if let Some(error) = &self.error {
            write!(f, "Critical task `{task_name}` panicked: `{error}`")
        } else {
            write!(f, "Critical task `{task_name}` panicked")
        }
    }
}

impl PanickedTaskError {
    fn new(task_name: &'static str, error: Box<dyn Any>) -> Self {
        let error = match error.downcast::<String>() {
            Ok(value) => Some(*value),
            Err(error) => match error.downcast::<&str>() {
                Ok(value) => Some(value.to_string()),
                Err(_) => None,
            },
        };

        Self { task_name, error }
    }
}

/// Determines how a task is spawned
enum TaskKind {
    /// Spawn the task to the default executor [Handle::spawn]
    Default,
    /// Spawn the task to the blocking executor [Handle::spawn_blocking]
    Blocking,
}

/// A Future that resolves when the shutdown event has been fired.
///
/// The [TaskManager](crate)
#[derive(Debug)]
pub struct LocalGracefulShutdown {
    _shutdown: Shutdown,
    _guard:    Option<LocalGracefulShutdownGuard>,
}

impl LocalGracefulShutdown {
    pub fn new(shutdown: Shutdown, guard: LocalGracefulShutdownGuard) -> Self {
        Self { _shutdown: shutdown, _guard: Some(guard) }
    }
}

#[derive(Debug)]
#[must_use = "if unused the task will not be gracefully shutdown"]
#[allow(unused)]
pub struct LocalGracefulShutdownGuard(Arc<AtomicUsize>);

impl LocalGracefulShutdownGuard {
    pub(crate) fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(counter)
    }
}
