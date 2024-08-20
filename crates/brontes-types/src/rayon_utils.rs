//! Utils around grouping operations to specific thread pools
//! this is done to allow for more precise control over our
//! par_iter allocations.
use std::sync::OnceLock;

/// NOTE: we exceed 100% due to the call operation flow.
/// we still expect to keep cpu usage near given value
pub fn init_thread_pools(max_tasks: usize) {
    // expensive ops, up to 200 ms
    let pricing_tasks = (max_tasks as f64 * 0.70) as usize + 1;
    // inspector runtime ~ 50ms
    let inspect_tasks = max_tasks;

    init_pricing_thread_pool(pricing_tasks);
    init_inspect_threadpool(inspect_tasks);
}

/// To use
/// ```ignore
/// execute_on!(target = ?, { code });
/// execute_on!(target = ?,  fn_call );
/// ```
/// where ? can be,
/// - pricing
/// - inspect
#[macro_export]
macro_rules! execute_on {
    (target=$t:tt, $block:block) => {
        execute_on!($t, $block)
    };
    (target=$t:tt, $($block:tt)+) => {
        execute_on!($t, { $($block)+ })
    };
    (pricing, $block:block) => {
        ::brontes_types::execute_on_pricing_thread_pool(|| $block)
    };
    (inspect, $block:block) => {
        ::brontes_types::execute_on_inspect_thread_pool(|| $block)
    };
    (async_inspect, $block:block) => {
        ::brontes_types::execute_on_inspect_thread_pool_async(move || $block)
    };
    (async_pricing, $block:block) => {
        ::brontes_types::execute_on_pricing_thread_pool_async(move || $block)
    };
}

/// ThreadPool for pricing operations
static RAYON_PRICING_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn init_pricing_thread_pool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("Pricing: {}", idx))
        .build()
        .unwrap();

    let _ = RAYON_PRICING_THREADPOOL.set(threadpool);
}

pub fn execute_on_pricing_thread_pool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_PRICING_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .install(op)
}

pub async fn execute_on_pricing_thread_pool_async<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    RAYON_PRICING_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .spawn(move || {
            let res = op();
            let _ = tx.send(res);
        });

    rx.await.unwrap()
}

/// ThreadPool for inspect operations
static RAYON_INSPECT_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn init_inspect_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("Inspect: {}", idx))
        .stack_size(1024 * 1024 * 32)
        .build()
        .unwrap();
    let _ = RAYON_INSPECT_THREADPOOL.set(threadpool);
}

pub fn execute_on_inspect_thread_pool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_INSPECT_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .install(op)
}

pub async fn execute_on_inspect_thread_pool_async<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    RAYON_INSPECT_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .spawn(move || {
            let res = op();
            let _ = tx.send(res);
        });

    rx.await.unwrap()
}
