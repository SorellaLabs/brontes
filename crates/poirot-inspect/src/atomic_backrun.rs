use crate::Inspector;
use poirot_labeller::Labeller;
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use reth_primitives::{Address};
use std::collections::hash_map::Entry;

use std::{collections::HashMap, sync::Arc};

pub struct AtomicBackrunInspector {
    db: Arc<Labeller>,
}

impl AtomicBackrunInspector {
    fn process_swaps(&self, all_swaps: Vec<Vec<Actions>>) {
        // address and there token delta's
        let mut deltas = HashMap::new();
        for swap in all_swaps.into_iter().flatten() {
            let Actions::Swap(swap) = swap else { unreachable!() };
            match deltas.entry(swap.call_address) {
                Entry::Occupied(mut o) => {
                    let inner: &mut HashMap<Address, i128> = o.get_mut();
                    match inner.entry(swap.token_in) {
                        Entry::Occupied(mut o) => {
                            *o.get_mut() -= swap.amount_in.to::<i128>();
                        }
                        Entry::Vacant(v) => {
                            v.insert(-swap.amount_in.to::<i128>());
                        }
                    }
                    match inner.entry(swap.token_out) {
                        Entry::Vacant(v) => {
                            v.insert(swap.amount_out.to::<i128>());
                        }
                        Entry::Occupied(mut o) => {
                            *o.get_mut() += swap.amount_out.to::<i128>();
                        }
                    }
                }
                Entry::Vacant(v) => {
                    let mut default = HashMap::default();
                    default.insert(swap.token_in, swap.amount_in.to::<i128>());
                    v.insert(default);
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {
        let _intersting_state = tree.inspect_all(|node| node.data.is_swap());
    }
}

pub struct AtomicArb {}
