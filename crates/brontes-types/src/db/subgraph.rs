use std::collections::HashMap;

use crate::price_graph::SubGraphEdge;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubGraphsEntry(pub HashMap<u64, Vec<SubGraphEdge>>);
