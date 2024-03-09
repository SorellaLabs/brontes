use std::sync::OnceLock;

/// Threadpool for operations on the block tree
pub static RAYON_TREE_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
/// Threadpool for pricing operations
pub static RAYON_PRICING_THREADPOOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

pub fn init_tree_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("BrontesTree: {}", idx))
        .build()
        .unwrap();
    RAYON_TREE_THREADPOOL.set(threadpool).unwrap();
}

pub fn execute_on_tree_threadpool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_TREE_THREADPOOL.get().unwrap().install(op)
}

pub fn init_pricing_threadpool(threads: usize) {
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("BrontesPricing: {}", idx))
        .build()
        .unwrap();

    RAYON_PRICING_THREADPOOL.set(threadpool).unwrap();
}

pub fn execute_on_pricing_threadpool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    RAYON_PRICING_THREADPOOL.get().unwrap().install(op)
}
