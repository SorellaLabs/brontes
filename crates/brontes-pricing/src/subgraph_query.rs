//! Functions for parallelizing graph fetches
use std::time::Duration;

use alloy_primitives::Address;
use brontes_types::{
    db::traits::{DBWriter, LibmdbxReader},
    FastHashSet, SubGraphEdge,
};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{types::PoolUpdate, GraphManager, Pair};

type GraphSeachParRes = (Vec<Vec<(Address, PoolUpdate)>>, Vec<Vec<NewGraphDetails>>);

pub fn graph_search_par<DB: DBWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    quote: Address,
    updates: Vec<PoolUpdate>,
) -> GraphSeachParRes {
    let (state, pools): (Vec<_>, Vec<_>) = updates
        .into_par_iter()
        .filter_map(|msg| {
            let pair = msg.get_pair(quote)?;
            let is_transfer = msg.is_transfer();

            let pair0 = Pair(pair.0, quote);
            let pair1 = Pair(pair.1, quote);
            let pair = Some(pair).filter(|_| !is_transfer).unwrap_or_default();

            let (state, path) = on_new_pool_pair(
                graph,
                msg,
                pair,
                (!graph.has_subgraph_goes_through(pair0, pair)).then_some(pair0),
                (!graph.has_subgraph_goes_through(pair1, pair)).then_some(pair1),
            );
            Some((state, path))
        })
        .unzip();

    (state, pools)
}

type ParStateQueryRes = Vec<StateQueryRes>;

pub struct RequeryPairs {
    pub pair:         Pair,
    pub goes_through: Pair,
    pub full_pair:    Pair,
    pub block:        u64,
    pub ignore_state: FastHashSet<Pair>,
    pub frayed_ends:  Vec<Address>,
}

pub struct NewGraphDetails {
    pub must_include:  Pair,
    pub complete_pair: Pair,
    pub pair:          Pair,
    pub extends_pair:  Option<Pair>,
    pub block:         u64,
    pub edges:         Vec<SubGraphEdge>,
}

pub struct StateQueryRes {
    pub pair:         Pair,
    pub block:        u64,
    pub edges:        Vec<Vec<SubGraphEdge>>,
    pub extends_pair: Option<Pair>,
    pub goes_through: Pair,
    pub full_pair:    Pair,
}

// already generated
pub fn par_state_query<DB: DBWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    pairs: Vec<RequeryPairs>,
) -> ParStateQueryRes {
    pairs
        .into_par_iter()
        .map(|RequeryPairs { pair, goes_through, full_pair, block, ignore_state, frayed_ends }| {
            // default extends,
            let default_extends_pair = graph.has_extension(&goes_through, pair.1);

            if frayed_ends.is_empty() {
                let (edges, extends_pair) = graph.create_subgraph(
                    block,
                    // if not zero, then we have a go, through
                    (!goes_through.is_zero()).then_some(goes_through),
                    pair,
                    ignore_state,
                    100,
                    Some(5),
                    Duration::from_millis(120),
                    default_extends_pair.is_some(),
                    None,
                );

                return StateQueryRes {
                    extends_pair,
                    pair,
                    block,
                    goes_through,
                    full_pair,
                    edges: vec![edges],
                }
            }

            StateQueryRes {
                edges: frayed_ends
                    .into_iter()
                    .zip(vec![pair.0].into_iter().cycle())
                    .collect_vec()
                    .into_par_iter()
                    .map(|(end, start)| {
                        graph
                            .create_subgraph(
                                block,
                                (!goes_through.is_zero()).then_some(goes_through),
                                Pair(start, end),
                                ignore_state.clone(),
                                0,
                                None,
                                Duration::from_millis(150),
                                default_extends_pair.is_some(),
                                None,
                            )
                            .0
                    })
                    .collect::<Vec<_>>(),
                full_pair,
                goes_through,
                pair,
                block,
                extends_pair: default_extends_pair,
            }
        })
        .collect::<Vec<_>>()
}

type NewPoolPair = (Vec<(Address, PoolUpdate)>, Vec<NewGraphDetails>);

fn on_new_pool_pair<DB: DBWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    msg: PoolUpdate,
    main_pair: Pair,
    pair0: Option<Pair>,
    pair1: Option<Pair>,
) -> NewPoolPair {
    let block = msg.block;

    let mut buf_pending = Vec::new();
    let mut path_pending = Vec::new();

    // add default pair to buffer to make sure that we price all pairs and apply the
    // state diff. we don't wan't to actually do a graph search for this pair
    // though.
    buf_pending.push((msg.get_pool_address(), msg));

    // add first pair
    if let Some(pair0) = pair0 {
        if let Some(path) = queue_loading_returns(graph, block, main_pair, pair0) {
            path_pending.push(path);
        }
    }

    // add second direction
    if let Some(pair1) = pair1 {
        if let Some(path) = queue_loading_returns(graph, block, main_pair.flip(), pair1) {
            path_pending.push(path);
        }
    }

    (buf_pending, path_pending)
}

fn queue_loading_returns<DB: DBWriter + LibmdbxReader>(
    graph: &GraphManager<DB>,
    block: u64,
    must_include: Pair,
    pair: Pair,
) -> Option<NewGraphDetails> {
    if pair.0 == pair.1 {
        return None
    }

    // if we can extend another graph and we don't have a direct pair with a quote
    // asset, then we will extend.
    let (mut n_pair, default_extend_to) = {
        if must_include.is_zero() {
            (pair, None)
        } else {
            graph
                .has_extension(&must_include, pair.1)
                .map(|ext| (must_include, Some(ext).filter(|_| must_include.1 != pair.1)))
                .unwrap_or((pair, None))
        }
    };

    Some({
        let (subgraph, actual_extends) = graph.create_subgraph(
            block,
            Some(must_include).filter(|m| !m.is_zero()),
            n_pair,
            FastHashSet::default(),
            100,
            Some(5),
            Duration::from_millis(69),
            default_extend_to.is_some(),
            Some(pair.1),
        );

        let extend_to = actual_extends
            .map(|actual| {
                n_pair.1 = actual.0;
                actual
            })
            .or(default_extend_to);

        NewGraphDetails {
            complete_pair: pair,
            pair: n_pair,
            must_include,
            block,
            edges: subgraph,
            extends_pair: extend_to,
        }
    })
}
