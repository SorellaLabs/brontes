use std::{
    fs,
    path::{Path, PathBuf},
};

use arrow::record_batch::RecordBatch;
use brontes_types::db::traits::LibmdbxReader;
use chrono::Local;
use eyre::{Error, Ok, Result, WrapErr};
use futures::try_join;
use parquet::{
    arrow::async_writer::AsyncArrowWriter, basic::Compression, file::properties::WriterProperties,
};
use tokio::fs::File;
use tracing::warn;

#[allow(dead_code)]
mod address_meta;
mod bundle_header;
mod mev_block;
pub mod utils;

use address_meta::address_metadata_to_record_batch;
use bundle_header::bundle_headers_to_record_batch;
use mev_block::mev_block_to_record_batch;

pub const DEFAULT_BUNDLE_DIR: &str = "data_exports/bundles";
pub const DEFAULT_BLOCK_DIR: &str = "data_exports/blocks";
pub const DEFAULT_METADATA_DIR: &str = "data_exports/address_metadata";

pub struct ParquetExporter<DB: LibmdbxReader> {
    pub start_block:      Option<u64>,
    pub end_block:        Option<u64>,
    pub parquet_dir_path: Option<String>,
    pub db:               &'static DB,
}

impl<DB> ParquetExporter<DB>
where
    DB: LibmdbxReader,
{
    pub fn new(
        start_block: Option<u64>,
        end_block: Option<u64>,
        parquet_dir_path: Option<String>,
        db: &'static DB,
    ) -> Self {
        Self { start_block, end_block, parquet_dir_path, db }
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
            warn!("No MEV blocks fetched for the given range.");
            return Ok(());
        }
        let block_batch =
            mev_block_to_record_batch(mev_blocks.iter().map(|mb| &mb.block).collect::<Vec<_>>())
                .wrap_err("Failed to convert MEV block data to record batch")?;

        let bundle_batch = bundle_headers_to_record_batch(
            mev_blocks
                .iter()
                .flat_map(|mb| mb.mev.iter().map(|bundle| &bundle.header))
                .collect::<Vec<_>>(),
        )
        .wrap_err("Failed to convert bundle headers to record batch")?;

        let block_path = if let Some(base_path) = &self.parquet_dir_path {
            let mut path = PathBuf::from(base_path);
            path.push("blocks");
            create_file_path(path)?
        } else {
            create_file_path(DEFAULT_BLOCK_DIR)?
        };

        let block_write = write_to_parquet_async(block_batch, block_path);

        let bundle_path = if let Some(base_path) = &self.parquet_dir_path {
            let mut path = PathBuf::from(base_path);
            path.push("bundles");
            create_file_path(path)?
        } else {
            create_file_path(DEFAULT_BUNDLE_DIR)?
        };

        let bundle_write = write_to_parquet_async(bundle_batch, bundle_path);

        try_join!(block_write, bundle_write)
            .wrap_err("Failed to write MEV blocks and bundles to Parquet files")?;

        Ok(())
    }

    pub async fn export_address_metadata(&self) -> Result<(), Error> {
        let address_metadata = self
            .db
            .fetch_all_address_metadata()
            .expect("Failed to query address metadata table");

        let address_meta_batch = address_metadata_to_record_batch(address_metadata)
            .expect("Failed to convert Address Metadata to record batch");

        let metadata_path = if let Some(base_path) = &self.parquet_dir_path {
            let mut path = PathBuf::from(base_path);
            path.push("address_metadata");
            create_file_path(path)?
        } else {
            create_file_path(DEFAULT_METADATA_DIR)?
        };

        write_to_parquet_async(address_meta_batch, metadata_path)
            .await
            .expect("Failed to write address metadata to parquet file");

        Ok(())
    }
}

async fn write_to_parquet_async(record_batch: RecordBatch, file_path: PathBuf) -> Result<()> {
    let file = File::create(file_path.clone())
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

fn create_file_path<P: AsRef<Path>>(base_dir: P) -> Result<PathBuf, Error> {
    let now = Local::now();

    // M:D format for dir
    let date_str = now.format("%m/%d").to_string();
    // Hour:Minute:Second format for filename for example "14:30:01"
    let time_str = now.format("%H:%M:%S").to_string();

    let dir_path = PathBuf::from(base_dir.as_ref()).join(&date_str);

    fs::create_dir_all(&dir_path)?;

    let file_path = dir_path.join(format!(
        "{}_{}.parquet",
        date_str.replace('/', "-"),
        time_str.replace(':', "-")
    ));

    Ok(file_path)
}
