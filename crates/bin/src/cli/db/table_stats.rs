use std::{sync::Arc, time::Duration};

use clap::Parser;
use comfy_table::{Cell, Row, Table as ComfyTable};
use eyre::WrapErr;
use itertools::Itertools;
use reth_db::{mdbx, static_file::iter_static_files, DatabaseEnv, TableViewer, Tables};

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
