use std::sync::Arc;

use alloy_primitives::Address;
use brontes_database::libmdbx::{Libmdbx, LibmdbxReader};
use brontes_types::{
    mev::{Bundle, BundleData, BundleHeader},
    normalized_actions::Actions,
    tree::BlockTree,
};

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct LongTailInspector<'db, DB: LibmdbxReader> {
    _inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> LongTailInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { _inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for LongTailInspector<'_, DB> {
    type Result = Vec<Bundle>;

    async fn process_tree(
        &self,
        _tree: Arc<BlockTree<Actions>>,
        _metadata: Arc<Metadata>,
    ) -> Self::Result {
        return vec![]
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
