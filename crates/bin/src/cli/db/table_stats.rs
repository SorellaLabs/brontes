use std::{path::Path, rc::Rc, sync::Arc, time::Duration};

use boyer_moore_magiclen::BMByte;
use clap::Parser;
use comfy_table::{Cell, Row, Table as ComfyTable};
use eyre::{Result, WrapErr};
use human_bytes::human_bytes;
use itertools::Itertools;
use reth_db::{
    cursor::{DbCursorRO, DbDupCursorRO},
    database::Database,
    mdbx, open_db_read_only,
    static_file::iter_static_files,
    table::{Decode, Decompress, DupSort, Table, TableRow},
    transaction::{DbTx, DbTxMut},
    DatabaseEnv, DatabaseError, RawTable, TableRawRow, TableViewer,
};
use reth_node_core::dirs::{ChainPath, DataDirPath};
/// Re-exported from `reth_node_core`, also to prevent a breaking change. See
/// the comment on the `reth_node_core::args` re-export for more details.
pub use reth_node_core::utils::*;
use reth_primitives::{fs, ChainSpec};
use reth_provider::ProviderFactory;
use tracing::info;

#[derive(Parser, Debug)]
/// The arguments for the `brontes db table-stats` command
pub struct Stats {
    /// Show only the total size for static files.
    #[arg(long, default_value_t = false)]
    detailed_sizes: bool,
}

impl Stats {
    /// Execute `db stats` command
    pub fn execute(self, db_path: String) -> eyre::Result<()> {
        let db_path = Path::new(&db_path);
        let chain = Arc::new(ChainSpec::default());

        let db = Arc::new(open_db_read_only(&db_path, Default::default())?);

        let mut statis_files_path = db_path.to_path_buf();
        statis_files_path.push("static_files");
        let provider_factory = ProviderFactory::new(db, chain.clone(), statis_files_path)?;

        let tool = DbTool::new(provider_factory, chain.clone())?;

        self.run(&tool)?;

        Ok(())
    }

    /// Execute `db stats` command
    fn run(self, tool: &DbTool<Arc<DatabaseEnv>>) -> eyre::Result<()> {
        let db_stats_table = self.db_stats_table(tool)?;
        println!("{db_stats_table}");

        Ok(())
    }

    fn db_stats_table(&self, tool: &DbTool<Arc<DatabaseEnv>>) -> eyre::Result<ComfyTable> {
        let mut table = ComfyTable::new();
        table.load_preset(comfy_table::presets::ASCII_MARKDOWN);
        table.set_header([
            "Table Name",
            "# Entries",
            "Branch Pages",
            "Leaf Pages",
            "Overflow Pages",
            "Total Size",
        ]);

        tool.provider_factory.db_ref().view(|tx| {
            let mut db_tables = brontes_db::libmdbx::tables::Tables::ALL
                .iter()
                .map(|table| table.name())
                .collect::<Vec<_>>();
            db_tables.sort();
            let mut total_size = 0;
            for db_table in db_tables {
                let table_db = tx
                    .inner
                    .open_db(Some(db_table))
                    .wrap_err("Could not open db.")?;

                let stats = tx
                    .inner
                    .db_stat(&table_db)
                    .wrap_err(format!("Could not find table: {db_table}"))?;

                // Defaults to 16KB right now but we should
                // re-evaluate depending on the DB we end up using
                // (e.g. REDB does not have these options as configurable intentionally)
                let page_size = stats.page_size() as usize;
                let leaf_pages = stats.leaf_pages();
                let branch_pages = stats.branch_pages();
                let overflow_pages = stats.overflow_pages();
                let num_pages = leaf_pages + branch_pages + overflow_pages;
                let table_size = page_size * num_pages;

                total_size += table_size;
                let mut row = Row::new();
                row.add_cell(Cell::new(db_table))
                    .add_cell(Cell::new(stats.entries()))
                    .add_cell(Cell::new(branch_pages))
                    .add_cell(Cell::new(leaf_pages))
                    .add_cell(Cell::new(overflow_pages))
                    .add_cell(Cell::new(human_bytes(table_size as f64)));
                table.add_row(row);
            }

            let max_widths = table.column_max_content_widths();
            let mut separator = Row::new();
            for width in max_widths {
                separator.add_cell(Cell::new("-".repeat(width as usize)));
            }
            table.add_row(separator);

            let mut row = Row::new();
            row.add_cell(Cell::new("Tables"))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(human_bytes(total_size as f64)));
            table.add_row(row);

            let freelist = tx.inner.env().freelist()?;
            let pagesize = tx
                .inner
                .db_stat(&mdbx::Database::freelist_db())?
                .page_size() as usize;
            let freelist_size = freelist * pagesize;

            let mut row = Row::new();
            row.add_cell(Cell::new("Freelist"))
                .add_cell(Cell::new(freelist))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(""))
                .add_cell(Cell::new(human_bytes(freelist_size as f64)));
            table.add_row(row);

            Ok::<(), eyre::Report>(())
        })??;

        Ok(table)
    }
}

/// Wrapper over DB that implements many useful DB queries.
#[derive(Debug)]
pub struct DbTool<DB: Database> {
    /// The provider factory that the db tool will use.
    pub provider_factory: ProviderFactory<DB>,
    /// The [ChainSpec] that the db tool will use.
    pub chain:            Arc<ChainSpec>,
}

impl<DB: Database> DbTool<DB> {
    /// Takes a DB where the tables have already been created.
    pub fn new(provider_factory: ProviderFactory<DB>, chain: Arc<ChainSpec>) -> eyre::Result<Self> {
        Ok(Self { provider_factory, chain })
    }

    /// Grabs the content of the table for the given key
    pub fn get<T: Table>(&self, key: T::Key) -> Result<Option<T::Value>> {
        self.provider_factory
            .db_ref()
            .view(|tx| tx.get::<T>(key))?
            .map_err(|e| eyre::eyre!(e))
    }

    /// Grabs the content of the DupSort table for the given key and subkey
    pub fn get_dup<T: DupSort>(&self, key: T::Key, subkey: T::SubKey) -> Result<Option<T::Value>> {
        self.provider_factory
            .db_ref()
            .view(|tx| tx.cursor_dup_read::<T>()?.seek_by_key_subkey(key, subkey))?
            .map_err(|e| eyre::eyre!(e))
    }

    /// Drops the database and the static files at the given path.
    pub fn drop(
        &mut self,
        db_path: impl AsRef<Path>,
        static_files_path: impl AsRef<Path>,
    ) -> Result<()> {
        let db_path = db_path.as_ref();
        info!(target: "reth::cli", "Dropping database at {:?}", db_path);
        fs::remove_dir_all(db_path)?;

        let static_files_path = static_files_path.as_ref();
        info!(target: "reth::cli", "Dropping static files at {:?}", static_files_path);
        fs::remove_dir_all(static_files_path)?;
        fs::create_dir_all(static_files_path)?;

        Ok(())
    }

    /// Drops the provided table from the database.
    pub fn drop_table<T: Table>(&mut self) -> Result<()> {
        self.provider_factory
            .db_ref()
            .update(|tx| tx.clear::<T>())??;
        Ok(())
    }
}
