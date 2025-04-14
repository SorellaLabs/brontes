use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::dex::BlockPrice,
    mev::{Bundle, BundleData, MevType, SearcherTx},
    normalized_actions::{accounting::ActionAccounting, Action},
    tree::BlockTree,
    ActionIter, BlockData, FastHashSet, MultiBlockData, ToFloatNearest, TreeSearchBuilder,
};
use itertools::multizip;
use malachite::{num::basic::traits::Zero, Rational};
use alloy_primitives::Address;

use super::{MAX_PROFIT, MIN_PROFIT};
use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct SearcherActivity<'db, DB: LibmdbxReader> {
    utils: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> SearcherActivity<'db, DB> {
    pub fn new(quote: Address, db: &'db DB, metrics: Option<OutlierMetrics>) -> Self {
        Self { utils: SharedInspectorUtils::new(quote, db, metrics) }
    }
}

impl<DB: LibmdbxReader> Inspector for SearcherActivity<'_, DB> {
    type Result = Vec<Bundle>;

    fn get_id(&self) -> &str {
        "SearcherActivity"
    }

    fn get_quote_token(&self) -> Address {
        self.utils.quote
    }

    fn inspect_block(&self, mut data: MultiBlockData) -> Self::Result {
        let block = data.per_block_data.pop().expect("no blocks");
        let BlockData { metadata, tree } = block;
        self.utils
            .get_metrics()
            .map(|m| {
                m.run_inspector(MevType::SearcherTx, || {
                    self.inspect_block_inner(tree.clone(), metadata.clone())
                })
            })
            .unwrap_or_else(|| self.inspect_block_inner(tree, metadata))
    }
}
impl<DB: LibmdbxReader> SearcherActivity<'_, DB> {
    fn inspect_block_inner(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        let search_args = TreeSearchBuilder::default()
            .with_actions([Action::is_transfer, Action::is_eth_transfer]);

        let (hashes, transfers): (Vec<_>, Vec<_>) = tree.clone().collect_all(search_args).unzip();
        let tx_info = tree.get_tx_info_batch(&hashes, self.utils.db);

        multizip((hashes, transfers, tx_info))
            .filter_map(|(tx_hash, transfers, info)| {
                if transfers.is_empty() {
                    return None
                }
                let info = info?;

                (info.searcher_eoa_info.is_some() || info.searcher_contract_info.is_some()).then(
                    || {
                        let deltas = transfers
                            .clone()
                            .into_iter()
                            .chain(info.get_total_eth_value().iter().cloned().map(Action::from))
                            .account_for_actions();

                        let mut searcher_address: FastHashSet<Address> = FastHashSet::default();
                        searcher_address.insert(info.eoa);
                        if let Some(mev_contract) = info.mev_contract {
                            searcher_address.insert(mev_contract);
                        }

                        let (rev_usd, mut has_dex_price) = if let Some(rev) =
                            self.utils.get_full_block_price(
                                BlockPrice::Lowest,
                                searcher_address,
                                &deltas,
                                metadata.clone(),
                            ) {
                            (Some(rev), true)
                        } else {
                            (Some(Rational::ZERO), false)
                        };

                        let gas_paid = metadata
                            .get_gas_price_usd(info.gas_details.gas_paid(), self.utils.quote);

                        let mut profit = rev_usd
                            .map(|rev| rev - gas_paid)
                            .filter(|_| has_dex_price)
                            .unwrap_or_default();

                        if profit >= MAX_PROFIT || profit <= MIN_PROFIT {
                            has_dex_price = false;
                            profit = Rational::ZERO;
                        }

                        let header = self.utils.build_bundle_header_searcher_activity(
                            vec![deltas],
                            vec![tx_hash],
                            &info,
                            profit.to_float(),
                            BlockPrice::Lowest,
                            &[info.gas_details],
                            metadata.clone(),
                            MevType::SearcherTx,
                            !has_dex_price,
                        );

                        Some(Bundle {
                            header,
                            data: BundleData::Unknown(SearcherTx {
                                block_number: metadata.block_num,
                                tx_hash,
                                gas_details: info.gas_details,
                                transfers: transfers
                                    .into_iter()
                                    .collect_action_vec(Action::try_transfer),
                            }),
                        })
                    },
                )?
            })
            .collect::<Vec<_>>()
    }
}
