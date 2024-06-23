use std::{env::temp_dir, path::PathBuf, str::FromStr};

use brontes_core::LibmdbxReadWriter;
use brontes_database::libmdbx::{merge_libmdbx_dbs, rclone_wrapper::BlockRangeList};
use brontes_types::{
    buf_writer::DownloadBufWriterWithProgress, unordered_buffer_map::BrontesStreamExt,
};
use clap::Parser;
use flate2::read::GzDecoder;
use futures::{stream::StreamExt, Stream};
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
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let client = reqwest::Client::new();
        let ranges_avail = self.get_available_ranges(&client).await?;
        let ranges_to_download = self.ranges_to_download(ranges_avail)?;

        let curl_queries = self
            .meets_space_requirement(&client, ranges_to_download, &brontes_db_endpoint)
            .await?;

        // download db tarball
        let multi_bar = MultiProgress::new();

        // ensure dir exists
        let mut download_dir = temp_dir();
        download_dir.push(format!("{}s", NAME));
        fs_extra::dir::create_all(&download_dir, false)?;

        futures::stream::iter(curl_queries)
            .map(|DbRequestWithBytes { url, size_bytes, file_name }| {
                let client = client.clone();
                let mb = multi_bar.clone();
                tracing::info!(?url, ?size_bytes, ?file_name);
                let mut download_dir = download_dir.clone();
                async move {
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
                    Self::handle_downloaded_file(&download_dir)?;

                    eyre::Ok(())
                }
            })
            .unordered_buffer_map(10, |f| tokio::spawn(f))
            .map(|s| s.map_err(eyre::Error::from))
            .collect_vec_transpose_double()
            .await??;

        tracing::info!(
            "all partitions downloaded, merging into the current db at: {}",
            brontes_db_endpoint
        );

        let final_db =
            LibmdbxReadWriter::init_db(brontes_db_endpoint, None, &ctx.task_executor, false)?;

        merge_libmdbx_dbs(final_db, &download_dir, ctx.task_executor)?;
        tracing::info!("cleaning up tmp libmdbx partitions");
        fs_extra::dir::remove(download_dir)?;

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
        let storage_available = fs2::free_space(brontes_db_endpoint)?;

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

        fs_extra::file::remove(tarball_location)?;

        Ok(())
    }
}

pub struct DbRequestWithBytes {
    pub url:        String,
    pub file_name:  String,
    pub size_bytes: u64,
}

impl<S> AsyncFlatten for S where S: Stream + Sized {}

trait AsyncFlatten: Stream {
    async fn collect_vec_transpose_double<T, E1, E2>(mut self) -> Result<Result<Vec<T>, E2>, E1>
    where
        Self: Sized + Unpin + Stream<Item = Result<Result<T, E2>, E1>>,
        E1: From<E2>,
    {
        let mut res = Vec::new();
        while let Some(next) = self.next().await {
            res.push(next??);
        }

        Ok(Ok(res))
    }
}
