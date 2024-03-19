//! Utils around grouping operations to specific threadpools
//! this is done to allow for more precise control over our
//! par_iter allocations.
use std::sync::OnceLock;

/// Takes all of our threadpools and initalizes them
/// tree gets 35% threads
/// pricing gets 70% threads
/// inspect gets 35% threads
/// NOTE: we exceed 100% due to the call operation flow.
/// we still expect to keep cpu usage near given value
pub fn init_threadpools(max_tasks: usize) {
    let tree_tasks = (max_tasks as f64 * 0.35) as usize + 1;
    let pricing_tasks = (max_tasks as f64 * 0.70) as usize + 1;
    let inspect_tasks = (max_tasks as f64 * 0.35) as usize + 1;

    init_tree_threadpool(tree_tasks);
    init_pricing_threadpool(pricing_tasks);
    init_inspect_threadpool(inspect_tasks);
}

/// To use
/// ```ignore
/// execute_on!(target = ?, { code });
/// execute_on!(target = ?,  fn_call );
/// ```
/// where ? can be,
/// - tree
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
    (tree, $block:block) => {
        $crate::execute_on_tree_threadpool(|| $block)
    };
    (pricing, $block:block) => {
        ::brontes_types::execute_on_pricing_threadpool(|| $block)
    };
    (inspect, $block:block) => {
        ::brontes_types::execute_on_inspect_threadpool(|| $block)
    };
}

/// ThreadPool for operations on the block tree
static RAYON_TREE_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn init_tree_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("Tree: {}", idx))
        .build()
        .unwrap();
    let _ = RAYON_TREE_THREADPOOL.set(threadpool);
}

pub fn execute_on_tree_threadpool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_TREE_THREADPOOL
        .get()
        .expect("threadpool not initialized")
        .install(op)
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
