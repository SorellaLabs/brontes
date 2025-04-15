//! Compute k-shortest paths using [Yen's search
//! algorithm](https://en.wikipedia.org/wiki/Yen%27s_algorithm).
use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
    hash::Hash,
    time::{Duration, SystemTime},
};

use brontes_types::{pair::Pair, FastHashMap, FastHashSet};
use pathfinding::num_traits::Zero;

pub use crate::graphs::dijkstras::*;

/// A representation of a path.
#[derive(Eq, PartialEq, Debug)]
struct Path<N: Eq + Hash + Clone, E: Eq + Hash + Clone, C: Zero + Ord + Copy> {
    /// The nodes along the path
    nodes:   Vec<N>,
    /// wieghts,
    weights: Vec<E>,
    /// The total cost of the path
    cost:    C,
}

impl<N, E, C> PartialOrd for Path<N, E, C>
where
    N: Eq + Hash + Clone,
    E: Eq + Hash + Clone,
    C: Zero + Ord + Copy,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<N, E, C> Ord for Path<N, E, C>
where
    N: Eq + Hash + Clone,
    E: Eq + Hash + Clone,
    C: Zero + Ord + Copy,
{
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare costs first, then amount of nodes
        let cmp = self.cost.cmp(&other.cost);
        match cmp {
            Ordering::Equal => self.nodes.len().cmp(&other.nodes.len()),
            _ => cmp,
        }
    }
}
/// Compute the k-shortest paths using the [Yen's search
/// algorithm](https://en.wikipedia.org/wiki/Yen%27s_algorithm).
///
/// The `k`-shortest paths starting from `start` up to a node for which
/// `success` returns `true` are computed along with their total cost. The
/// result is return as a vector of (path, cost).
///
/// - `start` is the starting node.
/// - `successors` returns a list of successors for a given node, along with the
///   cost of moving from the node to the successor. Costs MUST be positive.
/// - `success` checks whether the goal has been reached.
/// - `k` is the amount of paths requests, including the shortest one.
///
/// The returned paths include both the start and the end node and are ordered
/// by their costs starting with the lowest cost. If there exist less paths than
/// requested, only the existing ones (if any) are returned.
///
/// # Example
/// We will search the 3 shortest paths from node C to node H. See
/// <https://en.wikipedia.org/wiki/Yen's_algorithm#Example> for a visualization.
///
/// ```
/// use pathfinding::prelude::yen;
/// // Find 3 shortest paths from 'c' to 'h'
/// let paths = yen(
///     &'c',
///     |c| match c {
///         'c' => vec![('d', 3), ('e', 2)],
///         'd' => vec![('f', 4)],
///         'e' => vec![('d', 1), ('f', 2), ('g', 3)],
///         'f' => vec![('g', 2), ('h', 1)],
///         'g' => vec![('h', 2)],
///         'h' => vec![],
///         _ => panic!(""),
///     },
///     |c| *c == 'h',
///     3,
/// );
/// assert_eq!(paths.len(), 3);
/// assert_eq!(paths[0], (vec!['c', 'e', 'f', 'h'], 5));
/// assert_eq!(paths[1], (vec!['c', 'e', 'g', 'h'], 7));
/// assert_eq!(paths[2], (vec!['c', 'd', 'f', 'h'], 8));
///
/// // An example of a graph that has no path from 'c' to 'h'.
/// let empty = yen(
///     &'c',
///     |c| match c {
///         'c' => vec![('d', 3)],
///         'd' => vec![],
///         _ => panic!(""),
///     },
///     |c| *c == 'h',
///     2,
/// );
/// assert!(empty.is_empty());
/// ```
pub fn yen<N, C, E, FN, FS, FSE, PV>(
    start: &N,
    second: Option<&N>,
    successors: FN,
    success: FS,
    success_no_extends: FSE,
    path_value: PV,
    k: Option<usize>,
    max_iters: usize,
    extra_path_timeout: Duration,
    is_extension: bool,
    ends: &FastHashMap<N, Pair>,
) -> Vec<(Vec<E>, C)>
where
    N: Eq + Hash + Clone + Send + Sync,
    E: Clone + Default + Eq + Hash + Send + Sync,
    C: Zero + Ord + Copy + Send + Sync,
    FN: Fn(&N) -> Vec<(N, C)>,
    PV: Fn(&N, &N) -> E + Send + Sync,
    FS: Fn(&N) -> bool + Send + Sync,
    FSE: Fn(&N) -> bool + Send + Sync,
{
    let Some((e, n, c)) =
        dijkstra_internal(start, second, &successors, &path_value, &success, 25_000)
    else {
        return vec![];
    };

    // if we are extending another pair, we don't need any other routes as
    // the extension route has done most of the heavy lifting
    if is_extension || n.last().filter(|node| ends.contains_key(node)).is_some() {
        return vec![(e, c)]
    }

    // A vector containing our paths.
    let mut routes = vec![Path { nodes: n, weights: e, cost: c }];

    let mut visited = FastHashSet::default();
    let iter_k = k.unwrap_or(usize::MAX);

    // A min-heap to store our lowest-cost route candidate
    let mut k_routes = BinaryHeap::new();
    let start = SystemTime::now();
    for ki in 0..(iter_k - 1) {
        if routes.len() <= ki || routes.len() == iter_k {
            // We have no more routes to explore, or we have found enough.
            break
        }

        if SystemTime::now().duration_since(start).unwrap() > extra_path_timeout
            && k.map(|k| k >= routes.len()).unwrap_or(true)
        {
            tracing::debug!("timeout for extra routes hit");
            break
        }
        // Take the most recent route to explore new spurs.
        let previous = &routes[ki].nodes;
        let prev_weight = &routes[ki].weights;

        let k_routes_vec = (0..(previous.len() - 1))
            .filter_map(|i| {
                let spur_node = &previous[i];
                let root_path = &previous[0..i];
                let weight_root_path = &prev_weight[0..i];

                let mut filtered_edges = FastHashSet::default();
                for path in &routes {
                    if path.nodes.len() > i + 1
                        && &path.nodes[0..i] == root_path
                        && &path.nodes[i] == spur_node
                    {
                        filtered_edges.insert((&path.nodes[i], &path.nodes[i + 1]));
                    }
                }
                let filtered_nodes: FastHashSet<&N> = FastHashSet::from_iter(root_path);
                // We are creating a new successor function that will not return the
                // filtered edges and nodes that routes already used.
                let filtered_successor = |n: &N| {
                    successors(n)
                        .into_iter()
                        .filter(|(n2, _)| {
                            !filtered_nodes.contains(&n2) && !filtered_edges.contains(&(n, n2))
                        })
                        .collect::<Vec<_>>()
                };

                // Let us find the spur path from the spur node to the sink using.
                if let Some((values, spur_path, _)) = dijkstra_internal(
                    spur_node,
                    // if first node, then we have a forced second node.
                    second.filter(|_| i == 0),
                    &filtered_successor,
                    &path_value,
                    &success_no_extends,
                    max_iters,
                ) {
                    let nodes: Vec<N> = root_path.iter().cloned().chain(spur_path).collect();
                    let weights: Vec<E> = weight_root_path.iter().cloned().chain(values).collect();
                    // If we have found the same path before, we will not add it.
                    if !visited.contains(&nodes) {
                        // Since we don't know the root_path cost, we need to recalculate.
                        let cost = make_cost(&nodes, &successors);
                        let path = Path { nodes, weights, cost };
                        // Mark as visited
                        visited.insert(path.nodes.clone());
                        // Build a min-heap
                        return Some(Reverse(path))
                    }
                }
                None
            })
            .collect::<Vec<_>>();

        k_routes.extend(k_routes_vec);

        if let Some(k_route) = k_routes.pop() {
            let route = k_route.0;
            let cost = route.cost;
            routes.push(route);
            // If we have other potential best routes with the same cost, we can insert
            // them in the found routes since we will not find a better alternative.
            while routes.len() < iter_k {
                let Some(k_route) = k_routes.peek() else {
                    break;
                };
                if k_route.0.cost == cost {
                    let Some(k_route) = k_routes.pop() else {
                        break; // Cannot break
                    };
                    routes.push(k_route.0);
                } else {
                    break // Other routes have higher cost
                }
            }
        }
    }

    routes.sort_unstable();
    routes
        .into_iter()
        .map(|Path { weights, cost, .. }| (weights, cost))
        .collect()
}

fn make_cost<N, FN, IN, C>(nodes: &[N], successors: &FN) -> C
where
    N: Eq,
    C: Zero,
    FN: Fn(&N) -> IN,
    IN: IntoIterator<Item = (N, C)>,
{
    let mut cost = C::zero();
    for edge in nodes.windows(2) {
        for (n, c) in successors(&edge[0]) {
            if n == edge[1] {
                cost = cost + c;
            }
        }
    }
    cost
}
