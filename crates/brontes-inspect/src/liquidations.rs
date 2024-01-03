use std::sync::Arc;

use brontes_database::{Metadata, Pair};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{ClassifiedMev, Liquidation, MevType, SpecificMev},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    tree::TimeTree,
};
use reth_primitives::Address;

use crate::{shared_utils::SharedInspectorUtils, Inspector};

pub struct LiquidationInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> LiquidationInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for LiquidationInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        todo!()
    }
}
