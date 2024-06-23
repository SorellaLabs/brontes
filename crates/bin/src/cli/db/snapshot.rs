use std::{
    clone,
    env::{current_dir, temp_dir},
    path::PathBuf,
    str::FromStr,
};

use brontes_database::libmdbx::rclone_wrapper::BlockRangeList;
use brontes_types::buf_writer::DownloadBufWriterWithProgress;
use clap::Parser;
use filesize::file_real_size;
use flate2::read::GzDecoder;
use fs_extra::dir::CopyOptions;
use futures::stream::StreamExt;
use indicatif::MultiProgress;
use itertools::Itertools;
use reqwest::Url;
use tar::Archive;

use crate::runner::CliContext;

/// endpoint to check size of db snapshot
// const DOWNLOAD_PATH: &str = "brontes-db-latest.tar.gz";

const NAME: &str = "brontes-db-partition";
const FIXED_DB: &str = "full-range-tables";
const SIZE_PATH: &str = "byte-count.txt";
const RANGES_AVAILABLE: &str = "brontes-available-ranges.json";
const BYTES_TO_MB: u64 = 1_000_000;

#[derive(Debug, Parser)]
pub struct Snapshot {
    /// endpoint url
    #[arg(long, short, default_value = "https://pub-d0f2c20688264963b2c4ff2b4baa7c27.r2.dev")]
    pub endpoint:    Url,
    #[arg(long, short)]
    pub start_block: Option<u64>,
    #[arg(long, short)]
    pub end_block:   Option<u64>,
}

impl Snapshot {
    pub async fn execute(self, brontes_db_endpoint: String, _: CliContext) -> eyre::Result<()> {
        let client = reqwest::Client::new();
        let ranges_avail = self.get_available_ranges(&client).await?;
        let ranges_to_download = self.ranges_to_download(ranges_avail)?;

        let curl_queries = self
            .meets_space_requirement(&client, ranges_to_download, &brontes_db_endpoint)
            .await?;

        // download db tarball
        let multi_bar = MultiProgress::new();

        let err = futures::stream::iter(curl_queries)
            .map(|DbRequestWithBytes { url, size_bytes, file_name }| {
                let client = client.clone();
                let mb = multi_bar.clone();
                tracing::info!(?url, ?size_bytes, ?file_name);
                async move {
                    let mut download_dir = temp_dir();
                    download_dir.push(format!("{}s", NAME));
                    fs_extra::dir::create_all(&download_dir, false)?;

                    download_dir.push(file_name);

                    tracing::info!("creating file");
                    let file = tokio::fs::File::create(&download_dir).await?;

                    let stream = client.get(url).send().await?.bytes_stream();
                    DownloadBufWriterWithProgress::new(
                        Some(size_bytes),
                        stream,
                        file,
                        40 * 1024 * 1024,
                        &mb,
                    )
                    .await?;
                    tracing::info!("downloaded file");

                    Self::handle_downloaded_file(&download_dir)?;
                    tracing::info!("unpacked file");

                    eyre::Ok(())
                }
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        for e in err {
            e?;
        }

        tracing::info!(
            "finished downloading db. decompressing tar.gz and moving to final destination"
        );

        // self.handle_downloaded_file(&download_dir, &self.write_location)?;
        // tracing::info!("moved results to proper location");
        // fs_extra::file::remove(download_dir)?;
        // tracing::info!("deleted tarball");

        Ok(())
    }

    // returns a error if the data isn't available.
    // NOTE: assumes r2 data is continuous
    fn ranges_to_download(
        &self,
        ranges_avail: Vec<BlockRangeList>,
    ) -> eyre::Result<Vec<BlockRangeList>> {
        match (self.start_block, self.end_block) {
            (None, None) => Ok(ranges_avail),
            (Some(start), None) => {
                let ranges = ranges_avail
                    .into_iter()
                    .filter(|BlockRangeList { end_block, .. }| end_block >= &start)
                    .collect_vec();
                if ranges.is_empty() {
                    eyre::bail!(
                        "no data available for the set range: {:?}-{:?}",
                        self.start_block,
                        self.end_block
                    )
                }
                Ok(ranges)
            }
            (None, Some(end)) => {
                let ranges = ranges_avail
                    .into_iter()
                    .filter(|BlockRangeList { start_block, .. }| start_block <= &end)
                    .collect_vec();

                if ranges.is_empty() {
                    eyre::bail!(
                        "no data available for the set range: {:?}-{:?}",
                        self.start_block,
                        self.end_block
                    )
                }

                Ok(ranges)
            }
            (Some(start), Some(end)) => {
                let ranges = ranges_avail
                    .into_iter()
                    .filter(|BlockRangeList { start_block, end_block }| {
                        end_block >= &start && start_block <= &end
                    })
                    .collect_vec();

                if ranges.is_empty() {
                    eyre::bail!(
                        "no data available for the set range: {:?}-{:?}",
                        self.start_block,
                        self.end_block
                    )
                }

                Ok(ranges)
            }
        }
    }

    async fn get_available_ranges(
        &self,
        client: &reqwest::Client,
    ) -> eyre::Result<Vec<BlockRangeList>> {
        Ok(client
            .get(format!("{}{}", self.endpoint, RANGES_AVAILABLE))
            .send()
            .await?
            .json()
            .await?)
    }

    /// returns a error if there is not enough space remaining. If the overwrite
    /// db flag is enabled. Will delete the current db if that frees enough
    /// space
    async fn meets_space_requirement(
        &self,
        client: &reqwest::Client,
        ranges: Vec<BlockRangeList>,
        brontes_db_endpoint: &String,
    ) -> eyre::Result<Vec<DbRequestWithBytes>> {
        let mut new_db_size = 0u64;
        let mut res = vec![];
        for range in ranges {
            let url = format!(
                "{}{}-{}-{}-{}",
                self.endpoint, NAME, range.start_block, range.end_block, SIZE_PATH
            );
            let size = client.get(url).send().await?.text().await?;
            let size = u64::from_str(&size)?;
            res.push(DbRequestWithBytes {
                url:        format!(
                    "{}{}-{}-{}.tar.gz",
                    self.endpoint, NAME, range.start_block, range.end_block
                ),
                file_name:  format!("{}-{}-{}.tar.gz", NAME, range.start_block, range.end_block),
                size_bytes: size,
            });

            new_db_size += size;
        }

        // query 1 off table
        let url = format!("{}{}-{}-{}", self.endpoint, NAME, FIXED_DB, SIZE_PATH);
        let size = client.get(url).send().await?.text().await?;
        let size = u64::from_str(&size)?;

        res.push(DbRequestWithBytes {
            url:        format!("{}{}-{}.tar.gz", self.endpoint, NAME, FIXED_DB),
            file_name:  format!("{}-{}.tar.gz", NAME, FIXED_DB),
            size_bytes: size,
        });
        new_db_size += size;

        tracing::info!("new db size {}mb", new_db_size / BYTES_TO_MB);
        let storage_available = fs2::free_space(&brontes_db_endpoint)?;

        if storage_available >= new_db_size {
            Ok(res)
        } else {
            Err(eyre::eyre!(
                "not enough storage available. \nneeded: {}mb\navailable: {}mb",
                new_db_size / BYTES_TO_MB,
                storage_available / BYTES_TO_MB
            ))
        }
    }

    fn handle_downloaded_file(tarball_location: &PathBuf) -> eyre::Result<()> {
        let tar_gz = std::fs::File::open(tarball_location)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        let mut unpack = tarball_location.clone();
        unpack.pop();
        archive.unpack(&unpack)?;

        Ok(())
    }
}

pub struct DbRequestWithBytes {
    pub url:        String,
    pub file_name:  String,
    pub size_bytes: u64,
}
