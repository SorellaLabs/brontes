use std::collections::HashMap;

use brontes_database::libmdbx::{types::LibmdbxData, LibmdbxReader, LibmdbxWriter};
use brontes_types::db::{
    mev_block::{MevBlockWithClassified, MevType},
    searcher::SearcherStats,
};
use eyre::Result;

use crate::BrontesAnalytics;

impl<'a, T: TracingProvider> BrontesAnalytics {
    pub fn get_vertically_integrated_searchers(
        &self,
        start_block: u64,
        end_block: u64,
        mev_type: Option<Vec<MevType>>,
    ) -> Result<()> {
        let mut searcher_to_builder_map: HashMap<Address, (SearcherStats, Vec<Address>)> =
            HashMap::new();

        let mev_blocks = self.libmdbx.try_fetch_mev_blocks(start_block, end_block)?;

        for mev_block in mev_blocks {
            for bundle in mev_block.mev {
                if let Some(&types) = mev_type {
                    if !types.contains(&bundle.mev_type()) {
                        continue;
                    }
                }
                searcher_to_builder_map
                    .entry(bundle.get_searcher())
                    .or_default()
                    .push(mev_block.block.builder_address);
            }
        }

        let single_builder_searchers: HashMap<Address, Address> = searcher_to_builder_map
            .into_iter()
            .filter_map(|(searcher, builders)| {
                if builders.len() == 1 {
                    Some((searcher, builders[0]))
                } else {
                    None
                }
            })
            .collect();

        for (searcher, builder) in single_builder_searchers {
            let mut builder_info = self.libmdbx.try_fetch_builder_info(builder)?;

            if builder_info.searchers.contains(&searcher) {
                continue;
            } else {
                builder_info.searchers.push(searcher);

                self.libmdbx
                    .write_builder_info(builder_address, builder_info)
            }
        }

        Ok(())
    }
}
