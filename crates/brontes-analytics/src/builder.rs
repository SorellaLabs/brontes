use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxInit;
use brontes_types::{
    db::{builder::BuilderStats, searcher::SearcherStats},
    mev::bundle::MevType,
    traits::TracingProvider,
};
use eyre::Result;
use tracing::info;

use crate::BrontesAnalytics;

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub async fn get_vertically_integrated_searchers(
        &self,
        start_block: u64,
        end_block: u64,
        mev_type: Option<Vec<MevType>>,
    ) -> Result<(), eyre::Error> {
        let mut searcher_to_builder_map: HashMap<Address, (SearcherStats, HashSet<Address>)> =
            HashMap::new();
        let mut builder_map: HashMap<Address, BuilderStats> = HashMap::new();
        let mev_blocks = self.db.try_fetch_mev_blocks(start_block, end_block)?;

        for mev_block in mev_blocks {
            for bundle in mev_block.mev {
                if let Some(types) = &mev_type {
                    if !types.contains(&bundle.mev_type()) {
                        continue;
                    }
                }

                let (stats, builders) = searcher_to_builder_map
                    .entry(bundle.get_searcher_contract())
                    .or_insert_with(|| (SearcherStats::default(), HashSet::new()));

                stats.update_with_bundle(&bundle.header);

                builders.insert(mev_block.block.builder_address);
            }
            let builder_stats = builder_map
                .entry(mev_block.block.builder_address)
                .or_default();

            builder_stats.update_with_block(&mev_block.block);
        }

        for (searcher_address, (searcher_stats, _)) in &searcher_to_builder_map {
            self.db
                .write_searcher_stats(*searcher_address, searcher_stats.clone())
                .await?;
        }

        for (builder_address, builder_stats) in &builder_map {
            self.db
                .write_builder_stats(*builder_address, builder_stats.clone())
                .await?;
        }

        let single_builder_searchers: HashMap<Address, Address> = searcher_to_builder_map
            .into_iter()
            .filter_map(|(searcher, (searcher_stats, builders))| {
                if searcher_stats.bundle_count > 10 && builders.len() == 1 {
                    builders.iter().next().map(|builder| (searcher, *builder))
                } else {
                    None
                }
            })
            .collect();

        for (searcher, builder) in single_builder_searchers {
            info!("Identified vertically integrated searcher-builder pair: Searcher {:?}, Builder {:?}", searcher, builder);
            let mut builder_info = self.db.try_fetch_builder_info(builder)?;
            if !builder_info.searchers_eoa.contains(&searcher) {
                builder_info.searchers_contract.push(searcher);
                let _ = self.db.write_builder_info(builder, builder_info).await;
            }
        }

        Ok(())
    }
}
