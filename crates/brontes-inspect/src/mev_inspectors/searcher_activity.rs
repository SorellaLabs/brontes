use std::{collections::HashSet, sync::Arc};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::dex::PriceAt,
    mev::{Bundle, BundleData, MevType, SearcherTx},
    normalized_actions::{accounting::ActionAccounting, Actions},
    tree::BlockTree,
    ActionIter, ToFloatNearest, TreeSearchBuilder,
};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;

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
                if transfers.is_empty() {
                    return None
                }

                let info = tree.get_tx_info(tx_hash, self.utils.db)?;

                (info.searcher_eoa_info.is_some() || info.mev_contract.is_some()).then(|| {
                    let deltas = transfers.clone().into_iter().account_for_actions();

                    let mut searcher_address: HashSet<Address> = HashSet::new();
                    searcher_address.insert(info.eoa);
                    if let Some(mev_contract) = info.mev_contract {
                        searcher_address.insert(mev_contract);
                    }

                    let rev_usd = self.utils.get_deltas_usd(
                        info.tx_index,
                        PriceAt::After,
                        searcher_address,
                        &deltas,
                        metadata.clone(),
                    )?;
                    let gas_paid = metadata.get_gas_price_usd(info.gas_details.gas_paid());
                    let profit = rev_usd - gas_paid;

                    let header = self.utils.build_bundle_header(
                        vec![deltas],
                        vec![tx_hash],
                        &info,
                        profit.to_float(),
                        PriceAt::Average,
                        &[info.gas_details],
                        metadata.clone(),
                        MevType::Unknown,
                    );

                    Some(Bundle {
                        header,
                        data: BundleData::Unknown(SearcherTx {
                            tx_hash,
                            gas_details: info.gas_details,
                            transfers: transfers
                                .into_iter()
                                .collect_action_vec(Actions::try_transfer),
                        }),
                    })
                })?
            })
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
pub mod test {
    use alloy_primitives::hex;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_simple_searcher_tx() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.5).await;

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(Inspectors::SearcherActivity)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .needs_token(hex!("2559813bbb508c4c79e9ccce4703bcb1f149edd7").into())
            .with_expected_profit_usd(0.188588)
            .with_gas_paid_usd(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
