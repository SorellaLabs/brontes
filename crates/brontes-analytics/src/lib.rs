mod builder;
use alloy_primitives::Address;
#[allow(unused_imports)]
use brontes_database::{
    libmdbx::LibmdbxInit,
    parquet::{DEFAULT_BUILDER_INFO_DIR, DEFAULT_SEARCHER_INFO_DIR},
};
use brontes_types::{
    db::searcher::{Fund, ProfitByType, SearcherStats},
    mev::{Bundle, Mev, MevCount, MevType},
    traits::TracingProvider,
    FastHashMap, Protocol,
};
use eyre::Ok;
use polars::prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
pub struct BrontesAnalytics<T: TracingProvider, DB: LibmdbxInit> {
    pub db:             &'static DB,
    pub tracing_client: T,
}

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub fn new(db: &'static DB, tracing_client: T) -> Self {
        Self { db, tracing_client }
    }

    //TODO: make utils function that fetches most recent parquet file by date if no
    // path has been passed

    //TODO: Build polars expression that is equivalent to filter_bundle
    //TODO: Try and figure out how to add enum types instead of strings for enum

    //TODO: Profit by searcher, with:
    // Profit by mev type, mev type count, mev type average bribe
    // Profit by protocol

    pub async fn get_searcher_stats(
        &self,
        start_block: u64,
        end_block: u64,
        mev_types: Option<Vec<MevType>>,
        protocols: Option<Vec<Protocol>>,
        funds: Option<Vec<Fund>>,
    ) -> Result<(), eyre::Error> {
        let df = LazyFrame::scan_parquet(DEFAULT_BUILDER_INFO_DIR, Default::default())?;

        let _aggregate = df
            .lazy()
            .group_by([col("mev_contract")])
            .agg([
                col("tx_index").median().alias("median_tx_index"),
                col("eoa").unique().alias("unique_eoas"),
                col("profit_usd").sum().alias("total_profit"),
                col("profit_usd").mean().alias("profit_mean"),
                col("bribe_usd").sum().alias("total_bribed"),
                col("bribe_usd").mean().alias("bribe_mean"),
            ])
            .collect();

        let mut mev_stats = AggregateMevStats::default();

        let mev_blocks = self.db.try_fetch_mev_blocks(Some(start_block), end_block)?;

        let bundles: Vec<Bundle> = mev_blocks
            .into_par_iter()
            .filter(|mev_block| !mev_block.mev.is_empty())
            .map(|block| {
                block
                    .mev
                    .iter()
                    .filter_map(|bundle| self.filter_bundle(bundle, &mev_types, &protocols, &funds))
                    .collect::<Vec<Bundle>>()
            })
            .flatten()
            .collect();

        for bundle in &bundles {
            mev_stats.account(bundle)
        }

        Ok(())
    }

    pub fn filter_bundle(
        &self,
        bundle: &Bundle,
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

#[derive(Debug, Default, Clone)]
pub struct AggregateMevStats {
    pub mev_profit:   ProfitByType,
    pub total_bribed: ProfitByType,
    pub bundle_count: MevCount,
    searcher_stats:   FastHashMap<Address, SearcherStats>,
}

impl AggregateMevStats {
    pub fn account(&mut self, bundle: &Bundle) {
        self.mev_profit.account_by_type(&bundle.header);

        self.total_bribed.account_by_type(&bundle.header);
        self.bundle_count.increment_count(&bundle.header.mev_type);

        let stats = self
            .searcher_stats
            .entry(bundle.get_searcher_contract_or_eoa())
            .or_default();
        stats.update_with_bundle(&bundle.header);
    }
}
