use futures::ready;

use crate::{
    graphs::{Subgraph, VerificationOutcome},
    *,
};
pub enum PendingHeavyCalcs {
    SubgraphVerification(Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)>),
    StateQuery(ParStateQueryRes, bool),
    Rundown(Vec<RundownArgs>),
}

type CalcFut = Pin<Box<dyn Future<Output = (usize, PendingHeavyCalcs)> + Send>>;
pub type RundownArgs = (PairWithFirstPoolHop, Option<Pair>, u64, Vec<SubGraphEdge>, bool);

/// wrapper around a future unordered that allows us to query info about the
/// specific tasks
#[derive(Default)]
pub struct PendingTaskManager {
    id:      usize,
    futures: FuturesUnordered<CalcFut>,
    info:    FastHashMap<usize, Vec<TaskInfo>>,
}

impl PendingTaskManager {
    pub fn add_tasks(
        &mut self,
        tasks: Pin<Box<dyn Future<Output = PendingHeavyCalcs> + Send>>,
        info: impl Into<Vec<TaskInfo>>,
    ) {
        let task_id = self.id.overflowing_add(1).0;
        self.info.insert(task_id, info.into());

        self.futures
            .push(Box::pin(async move { (task_id, tasks.await) }));
    }

    pub fn tasks_for_block(&self, block: u64) -> usize {
        self.info
            .values()
            .flatten()
            .filter(|info| info.block == block)
            .count()
    }
}

impl Stream for PendingTaskManager {
    type Item = PendingHeavyCalcs;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let out = ready!(self.futures.poll_next_unpin(cx)).map(|(id, data)| {
            let _ = self.info.remove(&id);
            data
        });

        Poll::Ready(out)
    }
}

pub struct TaskInfo {
    pub block: u64,
    pub pair:  PairWithFirstPoolHop,
}

impl From<&(PairWithFirstPoolHop, u64)> for TaskInfo {
    fn from(value: &(PairWithFirstPoolHop, u64)) -> Self {
        TaskInfo { block: value.1, pair: value.0 }
    }
}
