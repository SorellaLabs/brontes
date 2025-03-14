use std::{
    fs::File,
    path::{Path, PathBuf},
};

use arrow::record_batch::RecordBatch;
use brontes_types::{
    db::traits::LibmdbxReader,
    mev::{BundleData, MevType},
};
use chrono::Local;
use eyre::{Error, Ok, Result, WrapErr};
use futures::future::try_join_all;
use parquet::{
    arrow::{async_writer::AsyncArrowWriter, ArrowWriter},
    basic::Compression,
    file::properties::WriterProperties,
};
use tracing::error;

use crate::Tables;

#[allow(dead_code)]
mod address_meta;
mod builder;
mod bundle_header;
mod mev_block;
mod mev_data;
mod normalized_actions;
mod searcher;
pub mod utils;

use address_meta::address_metadata_to_record_batch;
use builder::builder_info_to_record_batch;
use bundle_header::bundle_headers_to_record_batch;
use mev_block::mev_block_to_record_batch;
use mev_data::*;
use searcher::searcher_info_to_record_batch;

pub struct ParquetExporter<DB: LibmdbxReader> {
    pub start_block:   Option<u64>,
    pub end_block:     Option<u64>,
    pub base_dir_path: Option<String>,
    pub db:            &'static DB,
}

impl<DB> ParquetExporter<DB>
where
    DB: LibmdbxReader,
{
    pub fn new(
        start_block: Option<u64>,
        end_block: Option<u64>,
        base_dir_path: Option<String>,
        db: &'static DB,
    ) -> Self {
        Self { start_block, end_block, base_dir_path, db }
    }

    pub async fn export_mev_blocks(&self) -> Result<(), Error> {
        let mev_blocks = if let Some(end_block) = self.end_block {
            self.db
                .try_fetch_mev_blocks(self.start_block, end_block)
                .wrap_err("Failed to fetch MEV data from the database")?
        } else {
            self.db
                .fetch_all_mev_blocks(self.end_block)
                .wrap_err("Failed to fetch MEV data from the database")?
        };

        if mev_blocks.is_empty() {
            error!("No MEV blocks fetched for the given range.");
            return Err(Error::msg("No MEV blocks fetched for the given range."));
        }

        let mev_blocks_iter = mev_blocks.into_iter();
        let (
            blocks,
            bundle_headers,
            _cex_dex_arbs,
            atomic_arbs,
            jit,
            sandwich,
            jit_sandwich,
            searcher_tx,
            liquidation,
        ) = {
            let mut blocks = Vec::new();
            let mut bundle_headers = Vec::new();
            let mut cex_dex_arbs = Vec::new();
            let mut atomic_arbs = Vec::new();
            let mut jit = Vec::new();
            let mut sandwich = Vec::new();
            let mut jit_sandwich = Vec::new();
            let mut searcher_tx = Vec::new();
            let mut liquidation = Vec::new();

            for mb in mev_blocks_iter {
                blocks.push(mb.block);
                for bundle in mb.mev {
                    bundle_headers.push(bundle.header);
                    match bundle.data {
                        BundleData::CexDex(cex_dex) => cex_dex_arbs.push(cex_dex),
                        BundleData::AtomicArb(atomic_arb) => atomic_arbs.push(atomic_arb),
                        BundleData::Jit(jit_data) => jit.push(jit_data),
                        BundleData::Sandwich(sandwich_data) => sandwich.push(sandwich_data),
                        BundleData::JitSandwich(jit_sandwich_data) => {
                            jit_sandwich.push(jit_sandwich_data)
                        }
                        BundleData::Unknown(searcher_tx_data) => searcher_tx.push(searcher_tx_data),
                        BundleData::Liquidation(liquidation_data) => {
                            liquidation.push(liquidation_data)
                        }
                        _ => continue,
                    }
                }
            }

            (
                blocks,
                bundle_headers,
                cex_dex_arbs,
                atomic_arbs,
                jit,
                sandwich,
                jit_sandwich,
                searcher_tx,
                liquidation,
            )
        };

        let base_dir_path = self.base_dir_path.clone();

        let mut bundle_futures = Vec::new();

        if !blocks.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let block_batch = mev_block_to_record_batch(blocks)
                        .wrap_err("Failed to convert MEV block data to record batch")?;
                    sync_write_parquet(
                        block_batch,
                        get_path(base_dir_path, Tables::MevBlocks, None)?,
                    )
                }
            }));
        }

        /*if !_cex_dex_arbs.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let cex_dex_batch = cex_dex_to_record_batch(_cex_dex_arbs)
                        .wrap_err("Failed to convert CEX-DEX data to record batch")?;
                    sync_write_parquet(
                        cex_dex_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::CexDexTrades))?,
                    )
                }
            }));
        }*/

        if !atomic_arbs.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let atomic_arb_batch = atomic_arb_to_record_batch(atomic_arbs)
                        .wrap_err("Failed to convert AtomicArb data to record batch")?;
                    sync_write_parquet(
                        atomic_arb_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::AtomicArb))?,
                    )
                }
            }));
        }

        if !jit.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let jit_batch = jit_to_record_batch(jit)
                        .wrap_err("Failed to convert JIT data to record batch")?;
                    sync_write_parquet(
                        jit_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::Jit))?,
                    )
                }
            }));
        }

        if !sandwich.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let sandwich_batch = sandwich_to_record_batch(sandwich)
                        .wrap_err("Failed to convert Sandwich data to record batch")?;
                    sync_write_parquet(
                        sandwich_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::Sandwich))?,
                    )
                }
            }));
        }

        if !jit_sandwich.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let jit_sandwich_batch = jit_sandwich_to_record_batch(jit_sandwich)
                        .wrap_err("Failed to convert JIT Sandwich data to record batch")?;
                    sync_write_parquet(
                        jit_sandwich_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::JitSandwich))?,
                    )
                }
            }));
        }

        if !searcher_tx.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let searcher_tx_batch = searcher_tx_to_record_batch(searcher_tx)
                        .wrap_err("Failed to convert Searcher Tx data to record batch")?;
                    sync_write_parquet(
                        searcher_tx_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::SearcherTx))?,
                    )
                }
            }));
        }

        if !liquidation.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let liquidation_batch = liquidation_to_record_batch(liquidation)
                        .wrap_err("Failed to convert Liquidation data to record batch")?;
                    sync_write_parquet(
                        liquidation_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::Liquidation))?,
                    )
                }
            }));
        }

        if !bundle_headers.is_empty() {
            bundle_futures.push(tokio::task::spawn_blocking({
                let base_dir_path = base_dir_path.clone();
                move || {
                    let bundle_batch = bundle_headers_to_record_batch(bundle_headers)
                        .wrap_err("Failed to convert bundle headers to record batch")?;
                    sync_write_parquet(
                        bundle_batch,
                        get_path(base_dir_path, Tables::MevBlocks, Some(MevType::Unknown))?,
                    )
                }
            }));
        }

        try_join_all(bundle_futures).await?;
        Ok(())
    }

    pub async fn export_address_metadata(&self) -> Result<(), Error> {
        let address_metadata = self
            .db
            .fetch_all_address_metadata()
            .expect("Failed to query address metadata table");

        if address_metadata.is_empty() {
            error!("No MEV blocks fetched for the given range.");
            return Err(Error::msg("No MEV blocks fetched for the given range."));
        }

        let address_meta_batch = address_metadata_to_record_batch(address_metadata)
            .expect("Failed to convert Address Metadata to record batch");

        write_parquet(
            address_meta_batch,
            get_path(self.base_dir_path.clone(), Tables::AddressMeta, None)?,
        )
        .await
        .expect("Failed to write address metadata to parquet file");

        Ok(())
    }

    pub async fn export_searcher_info(&self) -> Result<(), Error> {
        let (eoa_info, contract_info) = self
            .db
            .fetch_all_searcher_info()
            .expect("Failed to query searcher eoa or contract table");

        if eoa_info.is_empty() && contract_info.is_empty() {
            error!("Searcher EOA & Contracts tables are empty.");
            return Err(Error::msg("No indexed searcher"));
        }

        let searcher_info_batch = searcher_info_to_record_batch(eoa_info, contract_info)
            .expect("Failed to convert Searcher Info to record batch");

        write_parquet(
            searcher_info_batch,
            get_path(self.base_dir_path.clone(), Tables::SearcherEOAs, None)?,
        )
        .await
        .expect("Failed to write searcher info to parquet file");

        Ok(())
    }

    pub async fn export_builder_info(&self) -> Result<(), Error> {
        let builder_info = self
            .db
            .fetch_all_builder_info()
            .expect("Failed to query builder table");

        if builder_info.is_empty() {
            error!("Builder table is empty.");
            return Err(Error::msg("No builder info"));
        }

        let builder_info_batch = builder_info_to_record_batch(builder_info)
            .expect("Failed to convert Searcher Info to record batch");

        write_parquet(
            builder_info_batch,
            get_path(self.base_dir_path.clone(), Tables::Builder, None)?,
        )
        .await
        .expect("Failed to write builder info to parquet file");

        Ok(())
    }
}

async fn write_parquet(record_batch: RecordBatch, file_path: PathBuf) -> Result<()> {
    let file = tokio::fs::File::create(file_path.clone())
        .await
        .wrap_err_with(|| format!("Failed to create file at path: {}", file_path.display()))?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = AsyncArrowWriter::try_new(file, record_batch.schema(), Some(props))
        .wrap_err("Failed to initialize Parquet writer")?;

    writer
        .write(&record_batch)
        .await
        .wrap_err("Failed to write record batch to Parquet file")?;

    writer
        .close()
        .await
        .wrap_err("Failed to close Parquet writer")?;

    Ok(())
}

fn sync_write_parquet(record_batch: RecordBatch, file_path: PathBuf) -> Result<()> {
    let file = File::create(file_path.clone())
        .wrap_err_with(|| format!("Failed to create file at path: {}", file_path.display()))?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = ArrowWriter::try_new(file, record_batch.schema(), Some(props))
        .wrap_err("Failed to initialize Parquet writer")?;

    writer
        .write(&record_batch)
        .wrap_err("Failed to write record batch to Parquet file")?;

    writer.close().wrap_err("Failed to close Parquet writer")?;

    Ok(())
}

pub fn get_path(
    custom_path: Option<String>,
    batch_type: Tables,
    mev_type: Option<MevType>,
) -> Result<PathBuf> {
    let base_path = custom_path
        .as_deref()
        .unwrap_or("../brontes-notebook/data/brontes-exports");

    let mut path = PathBuf::from(base_path);
    path.push(batch_type.get_default_path());

    if batch_type == Tables::MevBlocks && mev_type.is_none() {
        path.push("blocks");
    } else if let Some(mev_type) = mev_type {
        path.push("bundles");
        path.push(mev_type.get_parquet_path());
    }
    create_file_path(path)
}

pub fn create_file_path<P: AsRef<Path>>(base_dir: P) -> Result<PathBuf> {
    let now = Local::now();
    let date_str = now.format("%m-%d").to_string();
    let time_str = now.format("%H:%M").to_string();

    // Creates a flat directory structure
    // "data_exports/address_metadata/03-19"
    let dir_path = PathBuf::from(base_dir.as_ref()).join(date_str);
    std::fs::create_dir_all(&dir_path)?;

    let file_path = dir_path.join(format!("{}.parquet", time_str.replace(':', "-")));
    Ok(file_path)
}

impl Tables {
    pub fn get_default_path(&self) -> &'static str {
        match self {
            Tables::MevBlocks => DEFAULT_BLOCK_DIR,
            Tables::AddressMeta => DEFAULT_METADATA_DIR,
            Tables::SearcherEOAs => DEFAULT_SEARCHER_INFO_DIR,
            Tables::SearcherContracts => DEFAULT_SEARCHER_INFO_DIR,
            Tables::Builder => DEFAULT_BUILDER_INFO_DIR,
            _ => panic!("Unsupported table type"),
        }
    }
}
pub const DEFAULT_SEARCHER_STATS: &str = "searcher_stats";
pub const DEFAULT_BLOCK_DIR: &str = "mev";
pub const DEFAULT_METADATA_DIR: &str = "address_metadata";
pub const DEFAULT_SEARCHER_INFO_DIR: &str = "searcher_info";
pub const DEFAULT_BUILDER_INFO_DIR: &str = "builder-info";
