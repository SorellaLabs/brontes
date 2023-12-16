use std::{
    cmp::Ordering,
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, HashMap as StdHashMap,
    },
    hash::Hash,
    time::SystemTime,
};

use alloy_primitives::Address;
use petgraph::{
    algo::Measure,
    graph::UnGraph,
    prelude::*,
    unionfind::UnionFind,
    visit::{IntoEdges, NodeIndexable, VisitMap, Visitable},
};
use reth_primitives::revm_primitives::HashMap;
use tracing::info;

use crate::{DexQuotesMap, Pair, Quote};
type QuoteWithQuoteAsset<Q> = (Q, Address);

#[derive(Debug, Clone)]
pub struct PriceGraph<Q: Quote> {
    graph:  TrackableGraph<Address, QuoteWithQuoteAsset<Q>>,
    quotes: DexQuotesMap<Q>,
}

impl<Q> PriceGraph<Q>
where
    Q: Quote + Default,
{
    pub fn from_quotes_disjoint(quotes: DexQuotesMap<Q>) -> (Vec<Address>, Self) {
        let this = Self::from_quotes(quotes);
        (this.get_disjoint_token_edges(), this)
    }

    pub fn from_quotes(quotes: DexQuotesMap<Q>) -> Self {
        let graph = TrackableGraph::from_hash_map(
            quotes
                .0
                .clone()
                .into_iter()
                .map(|(k, v)| ((k.0, k.1), (v, k.1)))
                .collect(),
        );

        Self { quotes, graph }
    }

    pub fn add_new_pair(&mut self, pair: Pair, quote: Q) -> bool {
        let value = (quote, pair.1);
        self.graph.add_node((pair.0, pair.1), value)
    }

    /// grabs a token that is disjoint and queries for it
    pub fn get_disjoint_token_edges(&self) -> Vec<Address> {
        let mut vertex_sets = UnionFind::new(self.graph.graph.node_bound());
        for edge in self.graph.graph.edge_references() {
            let (a, b) = (edge.source(), edge.target());

            // union the two vertices of the edge
            vertex_sets.union(self.graph.graph.to_index(a), self.graph.graph.to_index(b));
        }
        let mut labels = vertex_sets.into_labeling();
        labels.sort_unstable();
        labels.dedup();

        if labels.len() == 1 {
            return vec![]
        }

        labels
            .into_iter()
            .map(|idx| *self.graph.index_to_addr.get(&idx).unwrap())
            .collect::<Vec<_>>()
    }

    pub fn has_token(&self, address: &Address) -> bool {
        self.graph.addr_to_index.contains_key(address)
    }

    // returns the quote for the given pair
    pub fn get_quote(&self, pair: &Pair) -> Option<Q> {
        // if we have a native pair use that
        //TODO: Change data structure to support multiple quotes for a single pair
        if let Some(quote) = self.quotes.get_quote(&pair) {
            return Some(quote.clone())
        }

        // generate the pair using the graph
        let base = pair.0;
        let quote = pair.1;

        // if base and quote are the same then its just 1
        if base == quote {
            return Some(Q::default())
        }

        let start_idx = self.graph.addr_to_index.get(&quote)?;
        let end_idx = self.graph.addr_to_index.get(&base)?;

        let path = dijkstra_path(&self.graph.graph, (*start_idx).into(), (*end_idx).into(), |_| 1)?;

        let mut res: Option<Q> = None;

        for i in 0..path.len() - 1 {
            let t0 = path[i];
            let t1 = path[i + 1];

            let edge = self.graph.graph.find_edge(t0, t1).unwrap();
            let (quote, quote_addr) = self.graph.graph.edge_weight(edge).unwrap();
            let index = *self.graph.addr_to_index.get(quote_addr).unwrap();
            let mut q = quote.clone();

            if index == t1.index() {
                q.inverse_price();
                if let Some(res) = &mut res {
                    *res *= q;
                } else {
                    res = Some(q);
                }
            } else if index == t0.index() {
                if let Some(res) = &mut res {
                    *res *= q;
                } else {
                    res = Some(q);
                }
            } else {
                unreachable!()
            }
        }

        info!(?pair, ?res, "graph gave us");
        res
    }
}

#[derive(Debug, Clone)]
pub struct TrackableGraph<K, V> {
    graph:         UnGraph<(), V, usize>,
    addr_to_index: HashMap<K, usize>,
    index_to_addr: HashMap<usize, K>,
}

impl<K, V> TrackableGraph<K, V>
where
    K: PartialEq + Hash + Eq + Clone + Copy,
    V: Clone,
{
    pub fn from_hash_map(map: HashMap<(K, K), V>) -> Self {
        let t0 = SystemTime::now();
        let mut graph = UnGraph::<(), V, usize>::default();

        let mut addr_to_index = HashMap::default();
        let mut index_to_addr = HashMap::default();
        let mut connections: HashMap<K, (usize, Vec<(K, usize, V)>)> = HashMap::new();

        for (pair, v) in map.clone() {
            // crate node if doesn't exist for addr or get node otherwise
            let addr0 = *addr_to_index
                .entry(pair.0)
                .or_insert(graph.add_node(()).index());
            index_to_addr.insert(addr0, pair.0);
            // crate node if doesn't exist for addr or get node otherwise
            let addr1 = *addr_to_index
                .entry(pair.1)
                .or_insert(graph.add_node(()).index());
            index_to_addr.insert(addr1, pair.1);

            //TODO: pretty sure we can do this without needing the addr
            //TODO: lifetime errors here
            // insert token0
            let e = connections.entry(pair.0).or_insert_with(|| (addr0, vec![]));

            // if we don't have this edge, then add it
            if !e.1.iter().map(|i| i.0).any(|addr| addr == pair.1) {
                e.1.push((pair.1, addr1, v.clone()));
            }

            // insert token1
            let e = connections.entry(pair.1).or_insert_with(|| (addr1, vec![]));
            // if we don't have this edge, then add it
            if !e.1.iter().map(|i| i.0).any(|addr| addr == pair.0) {
                e.1.push((pair.0, addr0, v));
            }
        }

        graph.extend_with_edges(connections.into_values().flat_map(|(node0, edges)| {
            edges
                .into_iter()
                .map(move |(_, adjacent, value)| (node0, adjacent, value))
        }));

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();

        info!(nodes=%graph.node_count(), edges=%graph.edge_count(), tokens=%addr_to_index.len(), "built graph in {}us", delta);

        Self { graph, addr_to_index, index_to_addr }
    }

    // returns false if there was a duplicate
    pub fn add_node(&mut self, pair: (K, K), value: V) -> bool {
        let node_0 = *self
            .addr_to_index
            .entry(pair.0)
            .or_insert(self.graph.add_node(()).index());
        let node_1 = *self
            .addr_to_index
            .entry(pair.1)
            .or_insert(self.graph.add_node(()).index());

        if self.graph.contains_edge(node_0.into(), node_1.into()) {
            return false
        }

        self.graph.add_edge(node_0.into(), node_1.into(), value);
        true
    }

    // fetches the path from start to end if it exists returning none if not
    pub fn get_path(&self, start: K, end: K) -> Option<Vec<K>> {
        let start_idx = self.addr_to_index.get(&start)?;
        let end_idx = self.addr_to_index.get(&end)?;

        Some(
            dijkstra_path(&self.graph, (*start_idx).into(), (*end_idx).into(), |_| 1)?
                .into_iter()
                .map(|k| {
                    self.index_to_addr
                        .get(&k.index())
                        .expect("Key not found in index_to_addr")
                        .clone()
                })
                .collect(),
        )
    }
}

/// returns the path from start to end if it exists where idx[0] == start
pub fn dijkstra_path<G, F, K>(
    graph: G,
    start: G::NodeId,
    goal: G::NodeId,
    mut edge_cost: F,
) -> Option<Vec<G::NodeId>>
where
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: Measure + Copy,
{
    let mut visited = graph.visit_map();
    let mut scores = StdHashMap::new();
    let mut predecessor = StdHashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = K::default();
    scores.insert(start, zero_score);
    visit_next.push(MinScored(zero_score, start));
    while let Some(MinScored(node_score, node)) = visit_next.pop() {
        if visited.is_visited(&node) {
            continue
        }
        if goal == node {
            break
        }

        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue
            }
            let next_score = node_score + edge_cost(edge);
            match scores.entry(next) {
                Occupied(ent) => {
                    if next_score < *ent.get() {
                        *ent.into_mut() = next_score;
                        visit_next.push(MinScored(next_score, next));
                        predecessor.insert(next.clone(), node.clone());
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score);
                    visit_next.push(MinScored(next_score, next));
                    predecessor.insert(next.clone(), node.clone());
                }
            }
        }
        visited.visit(node);
    }

    let mut path = Vec::new();

    let mut prev = predecessor.remove(&goal)?;
    path.push(goal);

    while let Some(next_prev) = predecessor.remove(&prev) {
        path.push(prev);
        prev = next_prev;
    }
    // add prev
    path.push(prev);
    // make start to finish
    path.reverse();

    Some(path)
}

/// `MinScored<K, T>` holds a score `K` and a scored object `T` in
/// a pair for use with a `BinaryHeap`.
///
/// `MinScored` compares in reverse order by the score, so that we can
/// use `BinaryHeap` as a min-heap to extract the score-value pair with the
/// least score.
///
/// **Note:** `MinScored` implements a total order (`Ord`), so that it is
/// possible to use float types as scores.
#[derive(Copy, Clone, Debug)]
pub struct MinScored<K, T>(pub K, pub T);

impl<K: PartialOrd, T> PartialEq for MinScored<K, T> {
    #[inline]
    fn eq(&self, other: &MinScored<K, T>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<K: PartialOrd, T> Eq for MinScored<K, T> {}

impl<K: PartialOrd, T> PartialOrd for MinScored<K, T> {
    #[inline]
    fn partial_cmp(&self, other: &MinScored<K, T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: PartialOrd, T> Ord for MinScored<K, T> {
    #[inline]
    fn cmp(&self, other: &MinScored<K, T>) -> Ordering {
        let a = &self.0;
        let b = &other.0;
        if a == b {
            Ordering::Equal
        } else if a < b {
            Ordering::Greater
        } else if a > b {
            Ordering::Less
        } else if a.ne(a) && b.ne(b) {
            // these are the NaN cases
            Ordering::Equal
        } else if a.ne(a) {
            // Order NaN less, so that it is last in the MinScore order
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}
