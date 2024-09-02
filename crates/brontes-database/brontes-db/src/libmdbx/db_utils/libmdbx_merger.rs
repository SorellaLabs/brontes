use std::path::PathBuf;

use brontes_types::BrontesTaskExecutor;
use fs_extra::dir::get_dir_content;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use rayon::iter::*;

use crate::{libmdbx::LibmdbxReadWriter, move_tables_to_partition, *};

pub fn merge_libmdbx_dbs(
    final_db: LibmdbxReadWriter,
    partition_db_folder: &PathBuf,
    executor: BrontesTaskExecutor,
) -> eyre::Result<()> {
    let files = get_dir_content(partition_db_folder)?;
    let multi = MultiProgress::default();
    // we can par this due to the single reader and not have any read locks.
    let directory_count = files.directories.len() as u64;
    let total_progress_bar = total_merge_bar(&multi, directory_count);

    files
        .directories
        .par_iter()
        .filter(|dir_name| *dir_name != partition_db_folder.to_str().unwrap())
        .filter_map(|path| LibmdbxReadWriter::init_db(path, None, &executor, false).ok())
        .try_for_each(|db| {
            move_tables_to_partition!(FULL_RANGE db, final_db, Some(multi.clone()),
            CexPrice,
            CexTrades,
            BlockInfo,
            MevBlocks,
            InitializedState,
            PoolCreationBlocks,
            TxTraces,
            AddressMeta,
            SearcherEOAs,
            SearcherContracts,
            Builder,
            AddressToProtocolInfo,
            TokenDecimals,
            DexPrice
            );
            total_progress_bar.inc(1);

            eyre::Ok(())
        })
}

pub fn total_merge_bar(mutli_bar: &MultiProgress, count: u64) -> ProgressBar {
    let progress_bar =
        ProgressBar::with_draw_target(Some(count), ProgressDrawTarget::stderr_with_hz(50));
    progress_bar.set_style(
        ProgressStyle::with_template(
            "{msg}\n[{elapsed_precise}] [{wide_bar:.green/red}] {pos}/{len} ({percent}%)",
        )
        .unwrap()
        .progress_chars("#>-")
        .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
            write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
        }),
    );
    progress_bar.set_message("Total Databases Merged");
    mutli_bar.add(progress_bar)
}
