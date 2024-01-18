use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{MevType, Sandwich, SpecificMev},
    normalized_actions::Actions,
    tree::{BlockTree, GasDetails, Node},
    ToFloatNearest,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::{Address, B256};
use tracing::info;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector};

pub struct LongTailInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> LongTailInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for LongTailInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        todo!()
    }
}

- atomically profitable 
- (leading zeros could be an indicator but I really doubt they would bother for long tail)
- fresh contract with repeated calls to the same function