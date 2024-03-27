mod builder;
use std::{
    fs::{self},
    path::PathBuf,
};

use brontes_database::{libmdbx::LibmdbxInit, parquet::create_file_path, Tables};
use brontes_types::{
    db::searcher::Fund,
    mev::{Bundle, Mev, MevType},
    traits::TracingProvider,
    Protocol,
};
use eyre::{Ok, Result};
use polars::prelude::*;

pub struct BrontesAnalytics<T: TracingProvider, DB: LibmdbxInit> {
    pub db:             &'static DB,
    pub tracing_client: T,
    pub custom_path:    Option<String>,
}

//TODO: make utils function that fetches most recent parquet file by date if no
// path has been passed

impl<T: TracingProvider, DB: LibmdbxInit> BrontesAnalytics<T, DB> {
    pub fn new(db: &'static DB, tracing_client: T, custom_path: Option<String>) -> Self {
        Self { db, tracing_client, custom_path }
    }

    pub async fn get_searcher_stats_by_mev_type(
        &self,
        _start_block: u64,
        _end_block: u64,
        _mev_types: Option<Vec<MevType>>,
        _protocols: Option<Vec<Protocol>>,
        _funds: Option<Vec<Fund>>,
    ) -> Result<(), eyre::Error> {
        let df = LazyFrame::scan_parquet(
            self.get_most_recent_parquet_file(Tables::MevBlocks, Some(MevType::Unknown))?,
            Default::default(),
        )?;

        let mut aggregate = df
            .lazy()
            .group_by([col("mev_contract"), col("mev_type")])
            .agg([
                col("tx_index").median().alias("median_tx_index"),
                col("eoa").unique().alias("unique_eoas"),
                col("profit_usd").sum().alias("total_profit"),
                col("profit_usd").mean().alias("profit_mean"),
                col("bribe_usd").sum().alias("total_bribed"),
                col("bribe_usd").mean().alias("bribe_mean"),
                col("mev_contract").count().alias("bundle_count"),
            ])
            .sort(
                "total_profit",
                SortOptions { descending: true, nulls_last: true, ..Default::default() },
            )
            .collect()?;

        print!("{:?}", aggregate);

        let path = get_analytics_path(None, "searcher_stats".to_string())?;
        let file = std::fs::File::create(&path)?;

        ParquetWriter::new(file).finish(&mut aggregate)?;

        Ok(())
    }

    // TODO: Optimise
    pub fn get_detailed_stats(
        &self,
        mev_types: Option<Vec<MevType>>,
        include_metadata: bool,
    ) -> Result<Vec<DataFrame>> {
        let bundle_header_path =
            self.get_most_recent_parquet_file(Tables::MevBlocks, Some(MevType::Unknown))?;
        let bundle_header_df = LazyFrame::scan_parquet(&bundle_header_path, Default::default())?;

        let mut joined_dfs = Vec::new();

        let mev_types = match mev_types {
            Some(types) => types,
            None => vec![
                MevType::CexDex,
                MevType::Sandwich,
                MevType::Jit,
                MevType::JitSandwich,
                MevType::Liquidation,
                MevType::AtomicArb,
                MevType::SearcherTx,
            ],
        };

        for mev_type in mev_types {
            let bundle_data_path =
                self.get_most_recent_parquet_file(Tables::MevBlocks, Some(mev_type))?;
            let bundle_data_df = LazyFrame::scan_parquet(&bundle_data_path, Default::default())?;

            let joined_df = match mev_type {
                MevType::CexDex | MevType::AtomicArb | MevType::SearcherTx => {
                    bundle_header_df.clone().join(
                        bundle_data_df,
                        [col("tx_hash")],
                        [col("tx_hash")],
                        JoinArgs::new(JoinType::Inner),
                    )
                }
                MevType::Sandwich | MevType::Jit | MevType::JitSandwich => {
                    bundle_header_df.clone().join(
                        bundle_data_df,
                        [col("tx_hash")],
                        [col("frontrun_tx_hashes").list().first()],
                        JoinArgs::new(JoinType::Inner),
                    )
                }
                MevType::Liquidation => bundle_header_df.clone().join(
                    bundle_data_df,
                    [col("tx_hash")],
                    [col("liquidation_tx_hash")],
                    JoinArgs::new(JoinType::Inner),
                ),
                MevType::Unknown => panic!("Unknown MEV type is not supported"),
            };

            if include_metadata {
                let address_metadata_path =
                    self.get_most_recent_parquet_file(Tables::AddressMeta, None)?;
                let address_metadata_df =
                    LazyFrame::scan_parquet(&address_metadata_path, Default::default())?
                        .collect()?;

                let final_df = joined_df
                    .join(
                        address_metadata_df.lazy(),
                        [col("mev_contract")],
                        [col("address")],
                        JoinArgs::new(JoinType::Inner),
                    )
                    .collect()?;

                joined_dfs.push(final_df);
            } else {
                joined_dfs.push(joined_df.collect()?);
            }
        }

        Ok(joined_dfs)
    }

    #[allow(dead_code)]
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

    fn get_most_recent_parquet_file(
        &self,
        batch_type: Tables,
        mev_type: Option<MevType>,
    ) -> Result<PathBuf> {
        let mut path = PathBuf::from(self.custom_path.as_deref().unwrap_or("brontes-exports"));
        path.push(batch_type.get_default_path());

        if batch_type == Tables::MevBlocks && mev_type.is_none() {
            path.push("blocks");
        } else if let Some(mev_type) = mev_type {
            path.push("bundles");
            path.push(mev_type.get_parquet_path());
        }
        let mut date_dirs: Vec<_> = fs::read_dir(path)?.filter_map(|entry| entry.ok()).collect();

        date_dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for date_dir in date_dirs {
            let mut entries: Vec<_> = fs::read_dir(date_dir.path())?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension().and_then(|ext| ext.to_str()) == Some("parquet")
                })
                .collect();

            // Sort the entries by filename, descending (most recent first)
            entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

            if let Some(entry) = entries.first() {
                return Ok(entry.path());
            }
        }

        Err(eyre::eyre!("No .parquet files found in the specified directory"))
    }
}

pub fn get_analytics_path(custom_path: Option<String>, analysis_path: String) -> Result<PathBuf> {
    let base_path = custom_path
        .as_deref()
        .unwrap_or("brontes-exports/analysis/");
    let mut path = PathBuf::from(base_path);
    path.push(analysis_path);
    create_file_path(path)
}
