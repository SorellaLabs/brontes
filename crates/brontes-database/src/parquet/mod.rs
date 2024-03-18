use arrow::record_batch::RecordBatch;
use brontes_types::{db::traits::LibmdbxReader, mev::BundleHeader};
use eyre::{Error, Result, WrapErr};
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

use bundle_header::bundle_headers_to_record_batch;
use mev_block::mev_block_to_record_batch;

pub struct ParquetExporter<DB: LibmdbxReader> {
    pub start_block: u64,
    pub end_block:   u64,
    pub db:          &'static DB,
}

impl<DB> ParquetExporter<DB>
where
    DB: LibmdbxReader,
{
    pub fn new(start_block: u64, end_block: u64, db: &'static DB) -> Self {
        Self { start_block, end_block, db }
    }

    pub async fn export_mev_blocks_and_bundles(&self) -> Result<(), Error> {
        let mev_blocks = self
            .db
            .try_fetch_mev_blocks(self.start_block, self.end_block)
            .wrap_err("Failed to fetch MEV data from the database")?;

        if mev_blocks.is_empty() {
            warn!("No MEV blocks fetched for the given range.");
            return Ok(());
        }

        tokio::fs::create_dir_all("db/parquet")
            .await
            .wrap_err("Failed to create the directory for Parquet files")?;

        for mev_block_with_classified in mev_blocks {
            let block_batch = mev_block_to_record_batch(vec![mev_block_with_classified.block])
                .wrap_err("Failed to convert MEV block data to record batch")?;
            let bundle_headers: Vec<BundleHeader> = mev_block_with_classified
                .mev
                .iter()
                .map(|bundle| bundle.header.clone())
                .collect();
            let bundle_batch = bundle_headers_to_record_batch(bundle_headers)
                .wrap_err("Failed to convert bundle headers to record batch")?;

            let block_write = write_to_parquet_async(block_batch, "db/parquet/block_table.parquet");
            let bundle_write =
                write_to_parquet_async(bundle_batch, "db/parquet/bundle_table.parquet");

            try_join!(block_write, bundle_write)
                .wrap_err("Failed to write MEV blocks and bundles to Parquet files")?;
        }

        Ok(())
    }
}

async fn write_to_parquet_async(record_batch: RecordBatch, file_path: &str) -> Result<()> {
    let file = File::create(file_path)
        .await
        .wrap_err_with(|| format!("Failed to create file at path: {}", file_path))?;

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
