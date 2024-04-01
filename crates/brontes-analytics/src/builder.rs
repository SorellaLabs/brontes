use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxInit;
use brontes_types::{mev::bundle::MevType, traits::TracingProvider, FastHashMap, FastHashSet};
use eyre::Result;
use tracing::info;

use crate::BrontesAnalytics;

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub async fn get_vertically_integrated_searchers(
        &self,
        _start_block: u64,
        _end_block: u64,
        _mev_type: Option<Vec<MevType>>,
    ) -> Result<(), eyre::Error> {
        todo!()
    }
    /*
        let mut searcher_to_builder_map: FastHashMap<
            Address,
            (SearcherStats, FastHashSet<Address>),
        > = FastHashMap::default();
        let mut builder_map: FastHashMap<Address, BuilderStats> = FastHashMap::default();
        let mev_blocks = self.db.try_fetch_mev_blocks(Some(start_block), end_block)?;

        for mev_block in mev_blocks {
            for bundle in mev_block.mev {
                if let Some(types) = &mev_type {
                    if !types.contains(&bundle.mev_type())
                        || bundle.get_searcher_contract().is_none()
                        || bundle.mev_type() == MevType::SearcherTx
                    {
                        continue
                    }
                }

                let (stats, builders) = searcher_to_builder_map
                    .entry(bundle.get_searcher_contract().unwrap())
                    .or_insert_with(|| (SearcherStats::default(), FastHashSet::default()));

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

        let single_builder_searchers: FastHashMap<Address, Address> = searcher_to_builder_map
            .into_iter()
            .filter_map(|(searcher, (searcher_stats, builders))| {
                if searcher_stats.bundle_count.mev_count > 10 && builders.len() == 1 {
                    builders.iter().next().map(|builder| (searcher, *builder))
                } else {
                    None
                }
            })
            .collect();

        for (searcher, builder) in single_builder_searchers {
            info!(
                "Identified vertically integrated searcher-builder pair: Searcher {:?}, Builder \
                 {:?}",
                searcher, builder
            );
            let mut builder_info = self
                .db
                .try_fetch_builder_info(builder)?
                .expect("Builder info not found");
            if !builder_info.searchers_eoas.contains(&searcher) {
                builder_info.searchers_contracts.push(searcher);
                let _ = self.db.write_builder_info(builder, builder_info).await;
            }
        }

        Ok(())
    }
     */
}
