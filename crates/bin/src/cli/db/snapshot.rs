use std::{env::current_dir, path::PathBuf, str::FromStr};

use brontes_types::buf_writer::DownloadBufWriterWithProgress;
use clap::Parser;
use filesize::file_real_size;
use flate2::read::GzDecoder;
use fs_extra::dir::CopyOptions;
use reqwest::Url;
use tar::Archive;

use crate::runner::CliContext;

/// endpoint to check size of db snapshot
const SIZE_PATH: &str = "db-size.txt";
const DOWNLOAD_PATH: &str = "brontes-db-latest.tar.gz";
const BYTES_TO_MB: u64 = 1_000_000;

/// the 3 files of libmdbx
const LIBMDBX_FILES: [&str; 3] = ["database.version", "mdbx.dat", "mdbx.lck"];

#[derive(Debug, Parser)]
pub struct Snapshot {
    /// endpoint url
    #[arg(long, short, default_value = "https://pub-e19b2b40b9c14ec3836e65c2c04590ec.r2.dev")]
    pub endpoint:       Url,
    /// where to write the database
    #[arg(long, short)]
    pub write_location: PathBuf,
    /// overwrite the database if it already exists
    /// in the write location
    #[arg(long, default_value = "false")]
    pub overwrite_db:   bool,
}

impl Snapshot {
    pub async fn execute(self, _: CliContext) -> eyre::Result<()> {
        fs_extra::dir::create_all(&self.write_location, false)?;

        let client = reqwest::Client::new();
        let db_size = self.meets_space_requirement(&client).await?;

        // delete db_location if exists
        if self.overwrite_db {
            if let Err(e) = self.try_delete_libmdbx_db() {
                tracing::warn!(err=%e, "error when trying to delete db from current location");
            }
        }

        // download db tarball
        let url = format!("{}{}", self.endpoint, DOWNLOAD_PATH);

        let mut download_dir = current_dir()?;
        download_dir.push("db-snapshot.tar.gz");

        let file = tokio::fs::File::create(&download_dir).await?;
        let stream = client.get(url).send().await?.bytes_stream();

        DownloadBufWriterWithProgress::new(Some(db_size), stream, file, 100 * 1024 * 1024).await?;
        tracing::info!("finished downloading db");
        self.handle_downloaded_file(&download_dir, &self.write_location)?;
        tracing::info!("moved results to proper location");
        fs_extra::file::remove(download_dir)?;
        tracing::info!("deleted tarball");

        Ok(())
    }

    /// returns a error if there is not enough space remaining. If the overwrite
    /// db flag is enabled. Will delete the current db if that frees enough
    /// space
    async fn meets_space_requirement(&self, client: &reqwest::Client) -> eyre::Result<u64> {
        let url = format!("{}{}", self.endpoint, SIZE_PATH);
        tracing::info!("trying url  {url}");
        let new_db_size = client.get(url).send().await?.text().await?;

        tracing::info!(?new_db_size);
        let new_db_size = u64::from_str(&new_db_size)?;

        tracing::info!("new db size {}mb", new_db_size / BYTES_TO_MB);

        let mut storage_available = fs2::free_space(&self.write_location)?;
        if self.overwrite_db {
            storage_available += self.libmdbx_file_size_bytes();
        }

        if storage_available >= new_db_size {
            Ok(new_db_size)
        } else {
            Err(eyre::eyre!(
                "not enough storage available. \nneeded: {}mb\navailable: {}mb",
                new_db_size / BYTES_TO_MB,
                storage_available / BYTES_TO_MB
            ))
        }
    }

    fn try_delete_libmdbx_db(&self) -> eyre::Result<()> {
        let mut write_location = self.write_location.clone();
        let mut report: Option<eyre::Report> = None;
        for ext in LIBMDBX_FILES {
            write_location.push(ext);
            if std::fs::metadata(&write_location).is_err() {
                tracing::warn!(path=?write_location, "file location doesn't exist");
                continue
            }

            let removed_file = std::fs::remove_file(&write_location);
            if let Err(e) = removed_file {
                report = Some(eyre::eyre!("{:?}", e))
            }
            write_location.pop();
        }

        if let Some(r) = report {
            return Err(r)
        }

        Ok(())
    }

    fn libmdbx_file_size_bytes(&self) -> u64 {
        let mut write_location = self.write_location.clone();
        LIBMDBX_FILES
            .iter()
            .filter_map(|ext| {
                write_location.push(ext);
                let res = file_real_size(&write_location).ok();
                write_location.pop();

                res
            })
            .sum::<u64>()
    }

    fn handle_downloaded_file(
        &self,
        tarball_location: &PathBuf,
        write_location: &PathBuf,
    ) -> eyre::Result<()> {
        let tar_gz = std::fs::File::open(tarball_location)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        let mut unpack = tarball_location.clone();

        unpack.pop();
        archive.unpack(&unpack)?;

        fs_extra::dir::move_dir(unpack, write_location, &CopyOptions::new())?;

        Ok(())
    }
}
