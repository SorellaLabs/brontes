use std::env;

use brontes_database::{
    libmdbx::{cursor::CompressedCursor, Libmdbx},
    CompressedTable, IntoTableKey, Tables,
};
use brontes_types::init_threadpools;
use clap::Parser;
use itertools::Itertools;
use reth_db::mdbx::RO;
use reth_interfaces::db::DatabaseErrorInfo;

#[derive(Debug, Parser)]
pub struct Export {
    /// that table to query
    //#[arg(long, short)]
    //pub table: Tables,
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will export until last entry
    #[arg(long, short)]
    pub end_block:   u64,
}

impl Export {
    pub async fn execute(self) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        init_threadpools(10);
        let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

        let tx = db.ro_tx()?;
    }
}
