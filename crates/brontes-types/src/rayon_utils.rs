//! Utils around grouping operations to specific threadpools
//! this is done to allow for more precise control over our
//! par_iter allocations.
use std::sync::OnceLock;

/// Takes all of our threadpools and initalizes them
/// pricing gets 70% threads
/// inspect gets 35% threads
/// NOTE: we exceed 100% due to the call operation flow.
/// we still expect to keep cpu usage near given value
pub fn init_threadpools(max_tasks: usize) {
    let pricing_tasks = (max_tasks as f64 * 0.70) as usize + 1;
    let inspect_tasks = (max_tasks as f64 * 0.40) as usize + 1;

    init_pricing_threadpool(pricing_tasks);
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
        ::brontes_types::execute_on_pricing_threadpool(|| $block)
    };
    (inspect, $block:block) => {
        ::brontes_types::execute_on_inspect_threadpool(|| $block)
    };
}

/// ThreadPool for pricing operations
static RAYON_PRICING_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn init_pricing_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("Pricing: {}", idx))
        .build()
        .unwrap();

    let _ = RAYON_PRICING_THREADPOOL.set(threadpool);
}

pub fn execute_on_pricing_threadpool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_PRICING_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .install(op)
}

/// ThreadPool for inspect operations
static RAYON_INSPECT_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn init_inspect_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("Inspect: {}", idx))
        .build()
        .unwrap();
    let _ = RAYON_INSPECT_THREADPOOL.set(threadpool);
}

pub fn execute_on_inspect_threadpool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_INSPECT_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .install(op)
}
