use std::sync::Arc;

use brontes_database::libmdbx::Libmdbx;
use brontes_types::{classified_mev::SpecificMev, normalized_actions::Actions, tree::BlockTree};
use reth_primitives::Address;

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, MetadataCombined};

pub struct LongTailInspector<'db> {
    _inner: SharedInspectorUtils<'db>,
}

impl<'db> LongTailInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { _inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for LongTailInspector<'_> {
    async fn process_tree(
        &self,
        _tree: Arc<BlockTree<Actions>>,
        _meta_data: Arc<MetadataCombined>,
    ) -> Vec<(ClassifiedMev, SpecificMev)> {
        todo!()
    }
}
//atomically profitable
// (leading zeros could be an indicator but I really doubt they would bother for
// long tail) fresh contract with repeated calls to the same function
// Address has interacted with tornado cash / is funded by tornado cash withdraw
// monero? other privacy bridges
// fixed float deposit addresses
// Check if there are any logs (mev bots shouldn't have any)
// coinbase opcode and transfers
// Selfdestruct opcode
// Any multicalls
// Flashloans yes and repeated calls could be too
// Check if etherscans api to check if bytecode is verified
// The more “f” in the bytecode, the more optimizer run has has been used, hence
// more
