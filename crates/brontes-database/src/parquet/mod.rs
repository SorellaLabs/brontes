use arrow::record_batch::RecordBatch;
use brontes_types::{db::mev_block::MevBlockWithClassified, mev::BundleHeader};
use eyre::{Result, WrapErr};
use futures::try_join;
use parquet::{
    arrow::async_writer::AsyncArrowWriter, basic::Compression, file::properties::WriterProperties,
};
use tokio::fs::File;

#[allow(dead_code)]
mod address_meta;
mod bundle_header;
mod mev_block;
pub mod utils;

use bundle_header::bundle_headers_to_record_batch;
use mev_block::mev_block_to_record_batch;

pub async fn export_mev_blocks_and_bundles(
    mev_blocks_with_classified: Vec<MevBlockWithClassified>,
) -> Result<()> {
    tokio::fs::create_dir_all("db/parquet").await?;

    for mev_block_with_classified in mev_blocks_with_classified {
        let block_batch = mev_block_to_record_batch(vec![mev_block_with_classified.block])?;
        let bundle_headers: Vec<BundleHeader> = mev_block_with_classified
            .mev
            .iter()
            .map(|bundle| bundle.header.clone())
            .collect();
        let bundle_batch = bundle_headers_to_record_batch(bundle_headers)?;

        let block_write = write_to_parquet_async(block_batch, "db/parquet/block_table.parquet");
        let bundle_write = write_to_parquet_async(bundle_batch, "db/parquet/bundle_table.parquet");

        try_join!(block_write, bundle_write)?;
    }

    Ok(())
}

async fn write_to_parquet_async(record_batch: RecordBatch, file_path: &str) -> Result<()> {
    let file = File::create(file_path)
        .await
        .wrap_err_with(|| format!("Failed to create file at path: {}", file_path))?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = AsyncArrowWriter::try_new(file, record_batch.schema(), Some(props))?;

    writer.write(&record_batch).await?;

    writer.close().await?;

    Ok(())
}
