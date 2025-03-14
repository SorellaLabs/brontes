use std::path::PathBuf;

use brontes_database::libmdbx::{
    rclone_wrapper::RCloneWrapper, LibmdbxInit, LibmdbxPartitioner, LibmdbxReadWriter,
    FULL_RANGE_NAME,
};
use clap::Parser;

use crate::runner::CliContext;

#[derive(Debug, Parser)]
pub struct R2Uploader {
    /// R2 Config Name
    #[clap(short, long)]
    r2_config_name:      String,
    /// Start Block
    #[clap(short, long)]
    start_block:         Option<u64>,
    /// Path to db partition folder
    #[clap(short, long, default_value = "/home/data/brontes-db-partitions/")]
    partition_db_folder: PathBuf,
    /// should also upload full db
    #[clap(short, long, default_value_t = false)]
    full_db:             bool,
    /// the amount of dbs to partition at a time
    #[clap(short, long, default_value_t = 10)]
    rayon_tasks:         usize,
}

impl R2Uploader {
    pub async fn execute(self, database_path: String, ctx: CliContext) -> eyre::Result<()> {
        let r2wrapper = RCloneWrapper::new(self.r2_config_name.clone()).await?;

        let db = LibmdbxReadWriter::init_db(&database_path, None, &ctx.task_executor, false)?;

        let start_block = if let Some(b) = self.start_block {
            b
        } else {
            tracing::info!("Grabbing most recent r2 snapshot");
            r2wrapper
                .get_most_recent_partition_block()
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(err=%e,"using databases first block");
                    db.get_db_range().expect("empty libmdbx").0
                })
        };

        if self.full_db {
            tracing::info!("uploading full database");
            if let Err(e) = r2wrapper
                .tar_ball_dir(&PathBuf::from(database_path), Some(FULL_RANGE_NAME))
                .await
            {
                tracing::error!(error=%e);
                return Ok(());
            }
            tracing::info!("uploading files completed");
        }

        tracing::info!("Partitioning new data into respective files");

        if let Err(e) = LibmdbxPartitioner::new(
            db,
            self.partition_db_folder.clone(),
            start_block,
            ctx.task_executor.clone(),
        )
        .execute(self.rayon_tasks)
        {
            tracing::error!(error=%e);
            return Ok(());
        }

        tracing::info!(
            "Partitioning complete, uploading files, this will take a while. ~10 min per partition"
        );

        if let Err(e) = r2wrapper
            .tar_ball_and_upload_files(self.partition_db_folder, start_block)
            .await
        {
            tracing::error!(error=%e);
            return Ok(());
        }

        Ok(())
    }
}
