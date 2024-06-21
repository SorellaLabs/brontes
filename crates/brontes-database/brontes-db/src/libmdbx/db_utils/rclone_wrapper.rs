use std::{fs::File, io::Write, path::PathBuf, process::Stdio, str::FromStr};

use eyre::eyre;
use fs_extra::dir::{get_dir_content, CopyOptions};
use futures::StreamExt;
use regex::Regex;
use tokio::process::Command;

use super::PARTITION_FILE_NAME;

/// rclone command wrapper
pub struct RCloneWrapper {
    config_name: &'static str,
}

impl RCloneWrapper {
    // ensures rclone is installed properly
    pub async fn new(config_name: &'static str) -> eyre::Result<Self> {
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
        let result = Command::new("rclone")
            .arg("tree")
            .arg(format!("{}:brontes_db", self.config_name))
            .stdout(Stdio::piped())
            .output()
            .await?;

        let string_result = String::from_utf8(result.stdout)?;
        let pattern = Regex::new(r"[\w-]+\.tar\.gz").unwrap();

        // Find the matches
        pattern
            .find_iter(&string_result)
            .filter_map(|files| {
                u64::from_str(files.as_str().split('-').last()?.split('.').next()?).ok()
            })
            .max()
            .ok_or_else(|| eyre!("no files found on r2"))
    }

    pub async fn upload_tarball(&self, directory_name: &str) {
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

    pub async fn tar_ball_and_upload_files(
        &self,
        partition_folder: PathBuf,
        start_block: u64,
    ) -> eyre::Result<()> {
        futures::stream::iter(
            get_dir_content(&partition_folder)?
                .directories
                .iter()
                .filter(|file_name| file_name.starts_with(PARTITION_FILE_NAME))
                .filter_map(|directory| {
                    let end_portion = directory.clone().split_off(PARTITION_FILE_NAME.len() + 1);
                    let file_start_block = u64::from_str(end_portion.split('-').next()?).unwrap();
                    (file_start_block >= start_block).then(|| {
                        let mut path = partition_folder.clone();
                        path.push(directory);
                        path
                    })
                }),
        )
        .map(|directory| async move {
            let directory_name = directory
                .components()
                .last()
                .unwrap()
                .as_os_str()
                .to_str()
                .unwrap();

            // move to the tmp dir for zipping and zip
            let copy = CopyOptions::new();
            // copy the data to tmp
            fs_extra::dir::copy(&directory, format!("/tmp/{directory_name}"), &copy);
            if !Command::new("tar")
                .arg("-czvf")
                .arg(format!("{directory_name}.tar.gz"))
                .arg("-C")
                .arg("/tmp/")
                .arg(directory_name)
                .spawn()
                .unwrap()
                .wait()
                .await
                .unwrap()
                .success()
            {
                panic!("failed to create tarball");
            }
            // get the tarball file size and write that
            let file_size =
                filesize::file_real_size(format!("/tmp/{directory_name}.tar.gz")).unwrap();
            let mut file = File::create("/tmp/{directory_name}-byte-count.txt").unwrap();
            write!(&mut file, "{}", file_size);

            // upload to the r2 bucket using rclone
            self.upload_tarball(directory_name).await;

            // c
        })
        .buffer_unordered(5)
        .collect::<Vec<_>>()
        .await;

        Ok(())
    }
}
