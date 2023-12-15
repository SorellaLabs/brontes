use std::{
    cmp::Ordering,
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, HashMap,
    },
    hash::Hash,
    time::SystemTime,
};

use alloy_primitives::Address;
use malachite::{num::basic::traits::One, Rational};
use petgraph::{
    algo::Measure,
    graph::UnGraph,
    prelude::*,
    visit::{IntoEdges, VisitMap, Visitable},
};
use tracing::info;

use crate::{Pair, Quote, QuotesMap};

type QuoteWithQuoteAsset = (Quote, Address);

#[derive(Debug, Clone)]
pub struct PriceGraph {
    graph:         UnGraph<(), QuoteWithQuoteAsset, usize>,
    addr_to_index: HashMap<Address, usize>,
    quotes:        QuotesMap,
}

impl PriceGraph {
    pub fn from_quotes(quotes: QuotesMap) -> Self {
        let t0 = SystemTime::now();
        let mut graph = UnGraph::<(), QuoteWithQuoteAsset, usize>::default();

        let mut addr_to_index = HashMap::default();
        let mut connections: HashMap<Address, (usize, Vec<(Address, usize, QuoteWithQuoteAsset)>)> =
            HashMap::new();

        for (pair, quote) in quotes.0.clone() {
            // crate node if doesn't exist for addr or get node otherwise
            let addr0 = *addr_to_index
                .entry(pair.0)
                .or_insert(graph.add_node(()).index());
            // crate node if doesn't exist for addr or get node otherwise
            let addr1 = *addr_to_index
                .entry(pair.1)
                .or_insert(graph.add_node(()).index());

            let quote = (quote, pair.1);

            //TODO: pretty sure we can do this without needing the addr

            // insert token0
            let e = connections.entry(pair.0).or_insert_with(|| (addr0, vec![]));
            // if we don't have this edge, then add it
            if !e.1.iter().map(|i| i.0).any(|addr| addr == pair.1) {
                e.1.push((pair.1, addr1, quote.clone()));
            }

            // insert token1
            let e = connections.entry(pair.1).or_insert_with(|| (addr1, vec![]));
            // if we don't have this edge, then add it
            if !e.1.iter().map(|i| i.0).any(|addr| addr == pair.0) {
                e.1.push((pair.0, addr0, quote));
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

        Self { quotes, graph, addr_to_index }
    }

    pub fn has_token(&self, address: &Address) -> bool {
        self.addr_to_index.contains_key(address)
    }

    // returns the quote for the given pair
    pub fn get_quote(&self, pair: &Pair) -> Option<Quote> {
        // if we have a native pair use that
        if let Some(quote) = self.quotes.get_quote(&pair) {
            return Some(quote.clone())
        }

        // generate the pair using the graph
        let base = pair.0;
        let quote = pair.1;

        // if base and quote are the same then its just 1
        if base == quote {
            let mut q = Quote::default();
            q.price = (Rational::ONE, Rational::ONE);
            return Some(q)
        }

        let start_idx = self.addr_to_index.get(&quote)?;
        let end_idx = self.addr_to_index.get(&base)?;

        let path = dijkstra_path(&self.graph, (*start_idx).into(), (*end_idx).into(), |_| 1)?;

        let mut res = Quote::default();

        for i in 0..path.len() - 1 {
            let t0 = path[i];
            let t1 = path[i + 1];

            let edge = self.graph.find_edge(t0, t1).unwrap();
            let (quote, quote_addr) = self.graph.edge_weight(edge).unwrap();
            let index = *self.addr_to_index.get(quote_addr).unwrap();
            let mut q = quote.clone();

            if index == t1.index() {
                q.inverse_price();
                if res.is_default() {
                    res = q;
                } else {
                    res *= q;
                }
            } else if index == t0.index() {
                if res.is_default() {
                    res = q;
                } else {
                    res *= q;
                }
            } else {
                unreachable!()
            }
        }

        info!(?pair, ?res, "graph gave us");

        Some(res)
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
    let mut scores = HashMap::new();
    let mut predecessor = HashMap::new();
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
