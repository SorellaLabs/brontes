use std::env;

use brontes_database::Tables;
use clap::Parser;
use eyre::Ok;

use crate::cli::load_database;

#[derive(Debug, Parser)]
pub struct Clear {
    /// Tables to clear
    #[arg(long, short, value_delimiter = ',')]
    pub tables: Vec<Tables>,
}

impl Clear {
    pub async fn execute(self) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = load_database(brontes_db_endpoint)?;

        macro_rules! clear_table {
    ($table:expr, $($tables:ident),+) => {
        match $table {
            $(
                Tables::$tables => {
                            libmdbx.0
                            .clear_table::<brontes_database::libmdbx::tables::$tables>().unwrap()
                }
            )+
        }
    };
}

        self.tables.iter().for_each(|table| {
            clear_table!(
                table,
                CexPrice,
                CexTrades,
                InitializedState,
                BlockInfo,
                DexPrice,
                MevBlocks,
                TokenDecimals,
                AddressToProtocolInfo,
                PoolCreationBlocks,
                Builder,
                BuilderStatistics,
                AddressMeta,
                SearcherEOAs,
                SearcherContracts,
                SearcherStatistics,
                SubGraphs,
                TxTraces
            )
        });

        Ok(())
    }
}
