use std::env;

use brontes_database::{libmdbx::Libmdbx, IntoTableKey, Tables};
use clap::Parser;

#[derive(Debug, Parser)]
pub struct AddToDb {
    /// that table to be queried
    #[arg(long, short)]
    pub table: Tables,
    // key of value
    #[arg(long, short)]
    pub key: String,
    // value
    #[arg(long, short)]
    pub value: String,
}

impl AddToDb {
    pub async fn execute(self) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

        macro_rules! write_to_table {
        ($table:expr, $($tables:ident),+ = $arg0:expr, $arg1:expr) => {
            match $table {
                $(
                    Tables::$tables => {
                        db.write_table(
                            &vec![
                            brontes_database::libmdbx::tables::$tables::into_table_data(
                                    $arg0,
                                    $arg1
                                )
                            ]
                        ).unwrap();
                    }
                )+
            }
        };
    }
        write_to_table!(
            self.table,
            CexPrice,
            BlockInfo,
            DexPrice,
            MevBlocks,
            AddressToProtocolInfo,
            TokenDecimals,
            SubGraphs,
            TxTraces,
            Builder,
            AddressMeta,
            Searcher,
            InitializedState,
            PoolCreationBlocks = &self.key,
            &self.value
        );

        Ok(())
    }
}
