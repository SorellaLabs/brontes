use std::{env::temp_dir, path::PathBuf, str::FromStr};

use brontes_database::libmdbx::{
    merge_libmdbx_dbs, rclone_wrapper::BlockRangeList, LibmdbxReadWriter, FULL_RANGE_NAME,
};
use brontes_types::{
    buf_writer::DownloadBufWriterWithProgress, unordered_buffer_map::BrontesStreamExt,
};
use clap::Parser;
use directories::UserDirs;
use flate2::read::GzDecoder;
use fs_extra::dir::{move_dir, CopyOptions};
use futures::{stream::StreamExt, Stream};
use indicatif::MultiProgress;
use itertools::Itertools;
use reqwest::Url;
use tar::Archive;

use crate::runner::CliContext;

const NAME: &str = "brontes-db-partition";
const FIXED_DB: &str = "full-range-tables";
const SIZE_PATH: &str = "byte-count.txt";
const RANGES_AVAILABLE: &str = "brontes-available-ranges.json";
const BYTES_TO_MB: u64 = 1_000_000;

#[derive(Debug, Parser)]
pub struct Snapshot {
    /// Snapshot endpoint
    #[arg(long, default_value = "https://data.brontes.xyz/")]
    pub endpoint:    Url,
    /// Optional start block
    #[arg(long, short)]
    pub start_block: Option<u64>,
    /// Optional end block
    #[arg(long, short)]
    pub end_block:   Option<u64>,
}

impl Snapshot {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let client = reqwest::Client::new();
        let ranges_avail = self.get_available_ranges(&client).await?;
        let ranges_to_download = self.ranges_to_download(ranges_avail)?;
        fs_extra::dir::create_all(&brontes_db_path, false)?;

        let curl_queries = self
            .meets_space_requirement(&client, ranges_to_download, &brontes_db_path)
            .await?;

        // download db tarball
        let multi_bar = MultiProgress::new();

        // ensure dir exists
        let mut download_dir = temp_dir();
        download_dir.push(format!("{}s", NAME));
        let mut cloned_download_dir = download_dir.clone();
        fs_extra::dir::create_all(&download_dir, false)?;

        ctx.task_executor
            .spawn_critical("download_streams", async move {
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
                    .await
                    .unwrap()
                    .unwrap();
            })
            .await?;

        if self.should_merge() {
            tracing::info!(
                "all partitions downloaded, merging into the current db at: {}",
                brontes_db_path
            );

            let final_db =
                LibmdbxReadWriter::init_db(brontes_db_path, None, &ctx.task_executor, false)?;

            let db = cloned_download_dir.clone();
            let ex = ctx.task_executor.clone();
            ctx.task_executor
                .spawn_blocking(async move {
                    merge_libmdbx_dbs(final_db, &db, ex).unwrap();
                })
                .await?;

            tracing::info!("cleaning up tmp libmdbx partitions");
            fs_extra::dir::remove(cloned_download_dir)?;
        } else {
            let mut home_dir = UserDirs::new()
                .expect("dirs failure")
                .home_dir()
                .to_path_buf();

            home_dir.push(FULL_RANGE_NAME);
            fs_extra::dir::create_all(&home_dir, true).expect("failed to create home dir folder");
            cloned_download_dir.push(FULL_RANGE_NAME);

            let opt = CopyOptions::new().overwrite(true);
            move_dir(cloned_download_dir, &home_dir, &opt)?;

            tracing::info!(download_path=?home_dir,"download of full db is finished");
        }

        Ok(())
    }

    // returns a error if the data isn't available.
    // NOTE: assumes r2 data is continuous
    fn ranges_to_download(&self, ranges_avail: Vec<BlockRangeList>) -> eyre::Result<RangeOrFull> {
        match (self.start_block, self.end_block) {
            (None, None) => Ok(RangeOrFull::Full),
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
                Ok(RangeOrFull::Range(ranges))
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

                Ok(RangeOrFull::Range(ranges))
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

                Ok(RangeOrFull::Range(ranges))
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
        ranges: RangeOrFull,
        brontes_db_path: &String,
    ) -> eyre::Result<Vec<DbRequestWithBytes>> {
        let mut new_db_size = 0u64;
        let mut res = vec![];
        match ranges {
            RangeOrFull::Full => {
                let url = format!("{}{}-{}", self.endpoint, FULL_RANGE_NAME, SIZE_PATH);
                let size = client.get(url).send().await?.text().await?;
                let size = u64::from_str(&size)?;
                res.push(DbRequestWithBytes {
                    url:        format!("{}{}.tar.gz", self.endpoint, FULL_RANGE_NAME),
                    file_name:  format!("{}.tar.gz", FULL_RANGE_NAME),
                    size_bytes: size,
                });

                new_db_size += size;
            }
            RangeOrFull::Range(ranges) => {
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
                        file_name:  format!(
                            "{}-{}-{}.tar.gz",
                            NAME, range.start_block, range.end_block
                        ),
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
            }
        }

        tracing::info!("new db size {}mb", new_db_size / BYTES_TO_MB);
        let storage_available = fs2::free_space(brontes_db_path)?;

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

    fn should_merge(&self) -> bool {
        self.start_block.is_some() || self.end_block.is_some()
    }
}

pub enum RangeOrFull {
    Full,
    Range(Vec<BlockRangeList>),
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
