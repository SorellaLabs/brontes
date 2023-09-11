use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc
};

use poirot_labeller::{Labeller, Metadata};
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use reth_primitives::Address;

use crate::{ClassifiedMev, Inspector};

pub struct AtomicBackrunInspector {}

impl AtomicBackrunInspector {
    fn process_swaps(&self, swaps: Vec<Actions>) -> Option<ClassifiedMev> {
        // address and there token delta's
        let mut deltas = HashMap::new();
        for swap in swaps.into_iter() {
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

        None
    }
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        meta_data: Arc<Metadata>
    ) -> Vec<ClassifiedMev> {
        let _intersting_state = tree.inspect_all(|node| node.data.is_swap());
        vec![]
    }
}

pub struct AtomicArb {}
