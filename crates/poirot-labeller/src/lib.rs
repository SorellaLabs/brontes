pub mod database;
use futures::{future::join_all, Future, FutureExt, StreamExt};
use poirot_core::executor::Executor;
pub struct Labeller {
    executor: Executor,
}

