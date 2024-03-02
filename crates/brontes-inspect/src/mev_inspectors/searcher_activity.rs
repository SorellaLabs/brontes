use std::{collections::HashSet, sync::Arc};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, Liquidation, MevType},
    normalized_actions::{accounting::ActionAccounting, Actions},
    tree::BlockTree,
    ActionIter, ToFloatNearest, TreeSearchBuilder, TxInfo,
};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{b256, Address};

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct SearcherActivity<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> SearcherActivity<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for SearcherActivity<'_, DB> {
    type Result = Vec<Bundle>;

    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Self::Result {
        let search_args = TreeSearchBuilder::default().with_actions([Actions::is_transfer]);

        let searcher_txs = tree.clone().collect_all(search_args).collect_vec();

        searcher_txs
            .into_par_iter()
            .filter_map(|(tx_hash, transfers)| {
                let info = tree.get_tx_info(tx_hash, self.utils.db)?;

                if info.searcher_eoa_info.is_some() || info.mev_contract.is_some() {
                    let deltas = transfers.into_iter().account_for_actions();

                    let mut searcher_address: HashSet<Address> = HashSet::new();
                    searcher_address.insert(info.eoa);
                    if let Some(mev_contract) = info.mev_contract {
                        searcher_address.insert(mev_contract);
                    }

                    let rev_usd = self.utils.get_deltas_usd(
                        info.tx_index,
                        PriceAt::After,
                        &searcher_address,
                        &deltas,
                        metadata.clone(),
                    )?;

                    Some(SearcherTx {
                        tx_hash,
                        searcher_addresses: searcher_address,
                        revenue_usd: rev_usd,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}
