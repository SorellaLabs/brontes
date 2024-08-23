use crate::graphs::{Subgraph, SubgraphVerificationState};

pub struct FrayedEnds {
    verification_state: SubgraphVerificationState,
    subgraph:           Subgraph,
}
