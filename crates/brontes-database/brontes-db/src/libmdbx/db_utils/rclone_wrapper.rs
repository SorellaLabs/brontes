use std::{fs::File, io::Write, path::PathBuf, process::Stdio, str::FromStr};

use eyre::eyre;
use fs_extra::dir::{get_dir_content, CopyOptions};
use futures::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::PARTITION_FILE_NAME;

/// rclone command wrapper
pub struct RCloneWrapper {
    config_name: String,
}

impl RCloneWrapper {
    // ensures rclone is installed properly
    pub async fn new(config_name: String) -> eyre::Result<Self> {
        if !Command::new("rclone")
            .arg("--version")
            .spawn()?
            .wait()
            .await?
            .success()
        {
            eyre::bail!("rclone is not installed on this computer, please fix")
        }

        Ok(Self { config_name })
    }

    pub async fn get_most_recent_partition_block(&self) -> eyre::Result<u64> {
        self.get_all_tarballs()
            .await?
            .into_iter()
            .filter_map(|files| u64::from_str(files.split('-').last()?.split('.').next()?).ok())
            .max()
            .ok_or_else(|| eyre!("no files found on r2"))
    }

    pub async fn get_blockrange_list(&self) -> eyre::Result<Vec<BlockRangeList>> {
        Ok(self
            .get_all_tarballs()
            .await?
            .into_iter()
            .filter_map(|mut file_names| {
                if file_names.ends_with("brontes-db-partition-full-range-tables.tar.gz") {
                    return None
                }

                tracing::info!(?file_names);

                let block_range_and_ext = file_names.split_off(PARTITION_FILE_NAME.len() + 1);
                let mut r = block_range_and_ext.split('.').next().unwrap().split('-');
                let start_block = u64::from_str(r.next().unwrap()).unwrap();
                let end_block = u64::from_str(r.next().unwrap()).unwrap();
                Some(BlockRangeList { end_block, start_block })
            })
            .collect::<Vec<_>>())
    }

    async fn get_all_tarballs(&self) -> eyre::Result<Vec<String>> {
        let result = Command::new("rclone")
            .arg("tree")
            .arg(format!("{}:brontes-db", self.config_name))
            .stdout(Stdio::piped())
            .output()
            .await?;

        let string_result = String::from_utf8(result.stdout)?;
        let pattern = Regex::new(r"[\w-]+\.tar\.gz").unwrap();

        // Find the matches
        Ok(pattern
            .find_iter(&string_result)
            .map(|file| file.as_str().to_string())
            .collect::<Vec<_>>())
    }

    async fn upload_tarball(&self, directory_name: &str) {
        if !Command::new("rclone")
            .arg("copy")
            .arg(format!("/tmp/{directory_name}.tar.gz"))
            .arg(format!("{}:brontes-db/", self.config_name))
            .arg("--s3-upload-cutoff=100M")
            .arg("--s3-chunk-size=100M")
            .spawn()
            .unwrap()
            .wait()
            .await
            .unwrap()
            .success()
        {
            panic!("failed to upload tarball");
        }

        if !Command::new("rclone")
            .arg("copy")
            .arg(format!("/tmp/{directory_name}-byte-count.txt"))
            .arg(format!("{}:brontes-db/", self.config_name))
            .spawn()
            .unwrap()
            .wait()
            .await
            .unwrap()
            .success()
        {
            panic!("failed to upload tarball");
        }
    }

    async fn upload_full_range_tables(&self, partition_folder: &PathBuf) -> eyre::Result<()> {
        let directory = PathBuf::from(
            get_dir_content(partition_folder)?
                .directories
                .iter()
                .find(|path| path.ends_with("brontes-db-partition-full-range-tables"))
                .expect("no full range table found"),
        );

        self.tar_ball_dir(&directory, None).await?;

        Ok(())
    }

    pub async fn tar_ball_dir(
        &self,
        directory: &PathBuf,
        new_name: Option<&str>,
    ) -> eyre::Result<()> {
        let mut directory_name = directory
            .components()
            .last()
            .unwrap()
            .as_os_str()
            .to_str()
            .unwrap();

        tracing::info!(?directory, ?directory_name);

        // move to the tmp dir for zipping and zip
        let copy = CopyOptions::new().overwrite(true);

        let tmp = format!("/tmp/{directory_name}");
        fs_extra::dir::create_all(&tmp, true).expect("failed to create tmp folder");
        tracing::info!(from=?directory, to=?tmp, "copying to tmp location");

        // copy the data to tmp
        fs_extra::dir::copy(directory, "/tmp/", &copy)?;

        // if we have a name change request,
        if let Some(new_directory_name) = new_name {
            std::fs::rename(tmp, format!("/tmp/{new_directory_name}"))?;
            directory_name = new_directory_name;
        }

        if !Command::new("tar")
            .arg("-I")
            .arg("pigz")
            .arg("-cf")
            .arg(format!("/tmp/{directory_name}.tar.gz"))
            .arg("-C")
            .arg("/tmp/")
            .arg(directory_name)
            .spawn()?
            .wait()
            .await?
            .success()
        {
            panic!("failed to create tarball");
        }

        // get the tarball file size and write that
        let file_size = filesize::file_real_size(format!("/tmp/{directory_name}.tar.gz"))?;

        let mut file = File::create(format!("/tmp/{directory_name}-byte-count.txt"))?;
        write!(&mut file, "{}", file_size).unwrap();

        // upload to the r2 bucket using rclone
        self.upload_tarball(directory_name).await;
        Ok(())
    }

    async fn update_block_range_file(&self) -> eyre::Result<()> {
        let ranges = self.get_blockrange_list().await?;
        let mut file = File::create("/tmp/brontes-available-ranges.json")?;
        let str = serde_json::to_string(&ranges)?;
        write!(&mut file, "{str}")?;

        if !Command::new("rclone")
            .arg("copy")
            .arg("/tmp/brontes-available-ranges.json")
            .arg(format!("{}:brontes-db/", self.config_name))
            .spawn()
            .unwrap()
            .wait()
            .await
            .unwrap()
            .success()
        {
            panic!("failed to upload available ranges");
        }

        Ok(())
    }

    pub async fn tar_ball_and_upload_files(
        &self,
        partition_folder: PathBuf,
        start_block: u64,
    ) -> eyre::Result<()> {
        tracing::info!(?partition_folder);
        self.upload_full_range_tables(&partition_folder).await?;

        futures::stream::iter(
            get_dir_content(&partition_folder)?
                .directories
                .iter()
                .filter(|path| {
                    *path != partition_folder.to_str().unwrap()
                        && !path.ends_with("brontes-db-partition-full-range-tables")
                })
                // ensure partition is in range
                .filter_map(|directory| {
                    let pathed = PathBuf::from(directory);

                    let directory = pathed
                        .components()
                        .last()
                        .unwrap()
                        .as_os_str()
                        .to_str()?
                        .to_string();

                    tracing::info!("tar balling directory {}", directory);
                    let end_portion = directory.clone().split_off(PARTITION_FILE_NAME.len() + 1);
                    tracing::info!(?end_portion);

                    let file_start_block = u64::from_str(end_portion.split('-').next()?).unwrap();
                    tracing::info!(%file_start_block);
                    (file_start_block >= start_block).then(|| {
                        let mut path = partition_folder.clone();
                        path.push(directory);
                        path
                    })
                }),
        )
        .map(|directory| async move {
            self.tar_ball_dir(&directory, None)
                .await
                .expect("failed to tarball dir")
        })
        .buffer_unordered(5)
        .collect::<Vec<_>>()
        .await;

        // upload ranges for downloader
        self.update_block_range_file().await?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlockRangeList {
    pub start_block: u64,
    pub end_block:   u64,
}
