//! Compute k-shortest paths using [Yen's search
//! algorithm](https://en.wikipedia.org/wiki/Yen%27s_algorithm).
use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, HashSet},
    hash::Hash,
};

use dashmap::DashSet;
use pathfinding::num_traits::Zero;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

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
/// - `success` checks weather the goal has been reached.
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

pub fn yen<N, C, E, FN, IN, FS, PV>(
    start: &N,
    mut successors: FN,
    mut success: FS,
    mut path_value: PV,
    k: usize,
    max_iters: usize,
) -> Vec<(Vec<E>, C)>
where
    N: Eq + Hash + Clone + Send + Sync,
    E: Clone + Default + Eq + Hash + Send + Sync,
    C: Zero + Ord + Copy + Send + Sync,
    FN: Fn(&N) -> IN + Send + Sync,
    PV: Fn(&N, &N) -> E + Send + Sync,
    IN: IntoIterator<Item = (N, C)>,
    FS: Fn(&N) -> bool + Send + Sync,
{
    let Some((e, n, c)) =
        dijkstra_internal(start, &mut successors, &mut path_value, &mut success, 20_000)
    else {
        return vec![];
    };

    let visited = DashSet::new();
    // A vector containing our paths.
    let mut routes = vec![Path { nodes: n, weights: e, cost: c }];
    // A min-heap to store our lowest-cost route candidate
    let mut k_routes = BinaryHeap::new();
    for ki in 0..(k - 1) {
        if routes.len() <= ki || routes.len() == k {
            // We have no more routes to explore, or we have found enough.
            break
        }
        // Take the most recent route to explore new spurs.
        let previous = &routes[ki].nodes;
        let prev_weight = &routes[ki].weights;

        let k_routes_vec = (0..(previous.len() - 1))
            .into_par_iter()
            .filter_map(|i| {
                let spur_node = &previous[i];
                let root_path = &previous[0..i];
                let weight_root_path = &prev_weight[0..i];

                let mut filtered_edges = HashSet::new();
                for path in &routes {
                    if path.nodes.len() > i + 1
                        && &path.nodes[0..i] == root_path
                        && &path.nodes[i] == spur_node
                    {
                        filtered_edges.insert((&path.nodes[i], &path.nodes[i + 1]));
                    }
                }
                let filtered_nodes: HashSet<&N> = HashSet::from_iter(root_path);
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
                    &filtered_successor,
                    &path_value,
                    &success,
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

        // Iterate over every node except the sink node.
        // for i in 0..(previous.len() - 1) {
        //     let spur_node = &previous[i];
        //     let root_path = &previous[0..i];
        //     let weight_root_path = &prev_weight[0..i];
        //
        //     let mut filtered_edges = HashSet::new();
        //     for path in &routes {
        //         if path.nodes.len() > i + 1
        //             && &path.nodes[0..i] == root_path
        //             && &path.nodes[i] == spur_node
        //         {
        //             filtered_edges.insert((&path.nodes[i], &path.nodes[i + 1]));
        //         }
        //     }
        //     let filtered_nodes: HashSet<&N> = HashSet::from_iter(root_path);
        //     // We are creating a new successor function that will not return the
        //     // filtered edges and nodes that routes already used.
        //     let mut filtered_successor = |n: &N| {
        //         successors(n)
        //             .into_iter()
        //             .filter(|(n2, _)| {
        //                 !filtered_nodes.contains(&n2) &&
        // !filtered_edges.contains(&(n, n2))             })
        //             .collect::<Vec<_>>()
        //     };
        //
        //     // Let us find the spur path from the spur node to the sink using.
        //     if let Some((values, spur_path, _)) = dijkstra_internal(
        //         spur_node,
        //         &mut filtered_successor,
        //         &mut path_value,
        //         &mut success,
        //         max_iters,
        //     ) {
        //         let nodes: Vec<N> =
        // root_path.iter().cloned().chain(spur_path).collect();         let
        // weights: Vec<E> = weight_root_path.iter().cloned().chain(values).collect();
        //         // If we have found the same path before, we will not add it.
        //         if !visited.contains(&nodes) {
        //             // Since we don't know the root_path cost, we need to
        // recalculate.             let cost = make_cost(&nodes, &mut
        // successors);             let path = Path { nodes, weights, cost };
        //             // Mark as visited
        //             visited.insert(path.nodes.clone());
        //             // Build a min-heap
        //             k_routes.push(Reverse(path));
        //         }
        //     }
        // }

        if let Some(k_route) = k_routes.pop() {
            let route = k_route.0;
            let cost = route.cost;
            routes.push(route);
            // If we have other potential best routes with the same cost, we can insert
            // them in the found routes since we will not find a better alternative.
            while routes.len() < k {
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
