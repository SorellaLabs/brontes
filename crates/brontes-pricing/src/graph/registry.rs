use std::collections::HashMap;

use alloy_primitives::Address;
use brontes_types::extra_processing::Pair;

use super::{subgraph::PairSubGraph, PoolState};

/// stores all sub-graphs and supports the update mechanisms
#[derive(Debug, Clone)]
pub struct SubGraphRegistry {
    /// tracks which tokens have a edge in the subgraph,
    /// this allows us to possibly insert a new node to a subgraph
    /// if it fits the criteria
    token_to_sub_graph: HashMap<Address, Vec<Pair>>,
    /// all currently known sub-graphs
    sub_graphs:         HashMap<Pair, PairSubGraph>,
    /// This is used to store a given pools tvl.
    /// we do this here so that all subpools just have a pointer
    /// to this data which allows us to not worry about updating all subgraphs
    /// when the tvl of a pool changes.
    /// pool address -> pool tvl
    edge_state:         HashMap<Address, PoolState>,
}

impl SubGraphRegistry {
    pub fn new(
        cached_subgraphs: HashMap<Pair, PairSubGraph>,
        token_to_sub_graph: HashMap<Address, Vec<Pair>>,
    ) -> Self {
        todo!()
    }
}
