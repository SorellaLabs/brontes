//! Functions for parallelizing graph fetches
use std::time::Duration;

use alloy_primitives::Address;
use brontes_types::{FastHashSet, SubGraphEdge};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{
    types::{PairWithFirstPoolHop, PoolUpdate},
    GraphManager, Pair,
};

const SUBGRAPH_TIMEOUT: Duration = Duration::from_millis(10);

type GraphSeachParRes = (Vec<Vec<(Address, PoolUpdate)>>, Vec<Vec<NewGraphDetails>>);

pub fn graph_search_par(
    graph: &GraphManager,
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

            let key0 = PairWithFirstPoolHop::from_pair_gt(pair0, pair);
            let key1 = PairWithFirstPoolHop::from_pair_gt(pair1, pair.flip());

            let (state, path) = on_new_pool_pair(
                graph,
                msg,
                pair,
                (!graph.has_subgraph_goes_through(key0)).then_some(pair0),
                (!graph.has_subgraph_goes_through(key1)).then_some(pair1),
            );
            Some((state, path))
        })
        .unzip();

    (state, pools)
}

type ParStateQueryRes = Vec<StateQueryRes>;

pub struct RequeryPairs {
    pub pair:         PairWithFirstPoolHop,
    pub extends_pair: Option<Pair>,
    pub block:        u64,
    pub ignore_state: FastHashSet<Pair>,
    pub frayed_ends:  Vec<Address>,
}

pub struct NewGraphDetails {
    pub pair:         PairWithFirstPoolHop,
    pub extends_pair: Option<Pair>,
    pub block:        u64,
    pub edges:        Vec<SubGraphEdge>,
}

pub struct StateQueryRes {
    pub pair:         PairWithFirstPoolHop,
    pub extends_pair: Option<Pair>,
    pub block:        u64,
    pub edges:        Vec<Vec<SubGraphEdge>>,
}

// already generated subgraph but need to fill in gaps
pub fn par_state_query(graph: &GraphManager, pairs: Vec<RequeryPairs>) -> ParStateQueryRes {
    pairs
        .into_par_iter()
        .map(|RequeryPairs { pair, block, ignore_state, frayed_ends, extends_pair }| {
            let gt = pair.get_goes_through();
            let full_pair = pair.get_pair();
            let default_extends_pair = graph.has_extension(&gt, full_pair.1);

            // if we have no direct extensions we are looking for, we will search the l
            if frayed_ends.is_empty() {
                let search_pair = extends_pair
                    .map(|extends| Pair(full_pair.0, extends.0))
                    .unwrap_or(full_pair);

                let (edges, extends_pair) = graph.create_subgraph(
                    block,
                    (!gt.is_zero()).then_some(gt),
                    search_pair,
                    ignore_state,
                    100,
                    None,
                    SUBGRAPH_TIMEOUT,
                    default_extends_pair.is_some(),
                    None,
                );

                return StateQueryRes { pair, extends_pair, edges: vec![edges], block }
            }

            StateQueryRes {
                edges: frayed_ends
                    .into_iter()
                    .zip(vec![pair.get_pair().0].into_iter().cycle())
                    .collect_vec()
                    .into_par_iter()
                    .map(|(end, start)| {
                        graph
                            .create_subgraph(
                                block,
                                (!gt.is_zero()).then_some(gt),
                                Pair(start, end),
                                ignore_state.clone(),
                                0,
                                None,
                                SUBGRAPH_TIMEOUT,
                                default_extends_pair.is_some(),
                                None,
                            )
                            .0
                    })
                    .collect::<Vec<_>>(),
                pair,
                block,
                extends_pair: default_extends_pair,
            }
        })
        .collect::<Vec<_>>()
}

type NewPoolPair = (Vec<(Address, PoolUpdate)>, Vec<NewGraphDetails>);

fn on_new_pool_pair(
    graph: &GraphManager,
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

fn queue_loading_returns(
    graph: &GraphManager,
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
            Some(10),
            SUBGRAPH_TIMEOUT,
            default_extend_to.is_some(),
            Some(pair.1),
        );

        let pair = PairWithFirstPoolHop::from_pair_gt(pair, must_include);

        let extend_to = actual_extends
            .inspect(|actual| {
                n_pair.1 = actual.0;
            })
            .or(default_extend_to);

        NewGraphDetails { pair, block, edges: subgraph, extends_pair: extend_to }
    })
}
