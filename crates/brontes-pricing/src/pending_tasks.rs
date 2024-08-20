use crate::{
    graphs::{Subgraph, VerificationOutcome},
    GraphSeachParRes, *,
};

pub enum PendingHeavyCalcs {
    DefaultCreate(u64, GraphSeachParRes),
    SubgraphVerification(Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)>),
}
