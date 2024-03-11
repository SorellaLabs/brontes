mod builder;
use std::sync::Arc;

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxInit;
use brontes_types::{
    db::searcher::{Fund, SearcherStats},
    mev::{Bundle, Mev, MevType},
    traits::TracingProvider,
    FastHashMap, Protocol,
};
use eyre::Ok;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
pub struct BrontesAnalytics<T: TracingProvider, DB: LibmdbxInit> {
    pub db:             &'static DB,
    pub tracing_client: T,
}

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub fn new(db: &'static DB, tracing_client: T) -> Self {
        Self { db, tracing_client }
    }

    pub fn searcher_stats(
        &self,
        start_block: u64,
        end_block: u64,
        mev_types: Option<Vec<MevType>>,
        protocols: Option<Vec<Protocol>>,
        funds: Option<Vec<Fund>>,
    ) -> Result<(), eyre::Error> {
        let mut searcher_stats: FastHashMap<Address, SearcherStats> = FastHashMap::default();

        let mev_blocks = self.db.try_fetch_mev_blocks(start_block, end_block)?;

        mev_blocks
            .par_iter()
            .filter(|mev_block| !mev_block.mev.is_empty())
            .map(|block| {
                block
                    .mev
                    .iter()
                    .filter_map(|bundle| self.filter_bundle(bundle, &mev_types, &protocols, &funds))
                    .for_each(|filtered_bundle| {
                        let stats = searcher_stats
                            .entry(filtered_bundle.get_searcher_contract_or_eoa())
                            .or_insert_with(|| (SearcherStats::default()));
                        stats.update_with_bundle(&filtered_bundle.header);
                    })
            });
        Ok(())
    }

    pub fn filter_bundle<'a>(
        &self,
        bundle: &'a Bundle,
        mev_types: &Option<Vec<MevType>>,
        protocols: &Option<Vec<Protocol>>,
        funds: &Option<Vec<Fund>>,
    ) -> Option<Bundle> {
        if let Some(mev_filter) = mev_types {
            if !mev_filter.contains(&bundle.header.mev_type) {
                return None;
            }
        }
        if let Some(protocols_filter) = protocols {
            let bundle_protocols = bundle.data.protocols();
            if !protocols_filter
                .iter()
                .any(|protocol| bundle_protocols.contains(protocol))
            {
                return None;
            }
        }

        if let Some(funds_filter) = funds {
            let (eoa_info, contract_info) = self
                .db
                .try_fetch_searcher_info(bundle.header.eoa, bundle.get_searcher_contract())
                .expect("Failed to query searcher table");

            match (eoa_info, contract_info) {
                (Some(eoa), Some(contract)) => {
                    if !funds_filter.contains(&eoa.fund) || !funds_filter.contains(&contract.fund) {
                        return None
                    }
                }
                (Some(eoa), None) => {
                    if !funds_filter.contains(&eoa.fund) {
                        return None
                    }
                }
                (None, Some(contract)) => {
                    if !funds_filter.contains(&contract.fund) {
                        return None
                    }
                }
                (None, None) => return None,
            }
        }

        Some(bundle.clone())
    }
}
