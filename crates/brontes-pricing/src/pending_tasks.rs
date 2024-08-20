use crate::{
    graphs::{Subgraph, VerificationOutcome},
    *,
};

pub enum PendingHeavyCalcs {
    SubgraphVerification(Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)>),
    StateQuery(ParStateQueryRes, bool),
    Rundown(Vec<(PairWithFirstPoolHop, Option<Pair>, u64, Vec<SubGraphEdge>, bool)>),
}
