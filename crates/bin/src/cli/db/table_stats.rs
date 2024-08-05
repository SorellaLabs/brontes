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
    mdbx,
    static_file::iter_static_files,
    table::{Decode, Decompress, DupSort, Table, TableRow},
    transaction::{DbTx, DbTxMut},
    DatabaseEnv, DatabaseError, RawTable, TableRawRow, TableViewer, Tables,
};
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
    pub fn execute(
        self,
        data_dir: ChainPath<DataDirPath>,
        tool: &DbTool<Arc<DatabaseEnv>>,
    ) -> eyre::Result<()> {
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
            let mut db_tables = Tables::ALL
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

    /// Grabs the contents of the table within a certain index range and places
    /// the entries into a [`HashMap`][std::collections::HashMap].
    ///
    /// [`ListFilter`] can be used to further
    /// filter down the desired results. (eg. List only rows which include
    /// `0xd3adbeef`)
    pub fn list<T: Table>(&self, filter: &ListFilter) -> Result<(Vec<TableRow<T>>, usize)> {
        let bmb = Rc::new(BMByte::from(&filter.search));
        if bmb.is_none() && filter.has_search() {
            eyre::bail!("Invalid search.")
        }

        let mut hits = 0;

        let data = self.provider_factory.db_ref().view(|tx| {
            let mut cursor = tx
                .cursor_read::<RawTable<T>>()
                .expect("Was not able to obtain a cursor.");

            let map_filter = |row: Result<TableRawRow<T>, _>| {
                if let Ok((k, v)) = row {
                    let (key, value) = (k.into_key(), v.into_value());

                    if key.len() + value.len() < filter.min_row_size {
                        return None
                    }
                    if key.len() < filter.min_key_size {
                        return None
                    }
                    if value.len() < filter.min_value_size {
                        return None
                    }

                    let result = || {
                        if filter.only_count {
                            return None
                        }
                        Some((
                            <T as Table>::Key::decode(&key).unwrap(),
                            <T as Table>::Value::decompress(&value).unwrap(),
                        ))
                    };

                    match &*bmb {
                        Some(searcher) => {
                            if searcher.find_first_in(&value).is_some()
                                || searcher.find_first_in(&key).is_some()
                            {
                                hits += 1;
                                return result()
                            }
                        }
                        None => {
                            hits += 1;
                            return result()
                        }
                    }
                }
                None
            };

            if filter.reverse {
                Ok(cursor
                    .walk_back(None)?
                    .skip(filter.skip)
                    .filter_map(map_filter)
                    .take(filter.len)
                    .collect::<Vec<(_, _)>>())
            } else {
                Ok(cursor
                    .walk(None)?
                    .skip(filter.skip)
                    .filter_map(map_filter)
                    .take(filter.len)
                    .collect::<Vec<(_, _)>>())
            }
        })?;

        Ok((data.map_err(|e: DatabaseError| eyre::eyre!(e))?, hits))
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

/// Filters the results coming from the database.
#[derive(Debug)]
pub struct ListFilter {
    /// Skip first N entries.
    pub skip:           usize,
    /// Take N entries.
    pub len:            usize,
    /// Sequence of bytes that will be searched on values and keys from the
    /// database.
    pub search:         Vec<u8>,
    /// Minimum row size.
    pub min_row_size:   usize,
    /// Minimum key size.
    pub min_key_size:   usize,
    /// Minimum value size.
    pub min_value_size: usize,
    /// Reverse order of entries.
    pub reverse:        bool,
    /// Only counts the number of filtered entries without decoding and
    /// returning them.
    pub only_count:     bool,
}

impl ListFilter {
    /// If `search` has a list of bytes, then filter for rows that have this
    /// sequence.
    pub fn has_search(&self) -> bool {
        !self.search.is_empty()
    }

    /// Updates the page with new `skip` and `len` values.
    pub fn update_page(&mut self, skip: usize, len: usize) {
        self.skip = skip;
        self.len = len;
    }
}
