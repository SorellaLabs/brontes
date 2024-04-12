use brontes_database::{libmdbx::Libmdbx, IntoTableKey, Tables};
use brontes_types::init_threadpools;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Insert {
    /// that table to be queried
    #[arg(long, short)]
    pub table: Tables,
    // key of value
    #[arg(long, short)]
    pub key:   String,
    // value
    #[arg(long, short)]
    pub value: String,
}

impl Insert {
    pub async fn execute(self, brontes_db_endpoint: String) -> eyre::Result<()> {
        let db = Libmdbx::init_db(brontes_db_endpoint, None)?;
        init_threadpools(10);

        macro_rules! write_to_table {
        ($table:expr, $($tables:ident),+ = $arg0:expr, $arg1:expr) => {
            match $table {
                $(
                    Tables::$tables => {
                        db.write_table(
                            &[
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
            CexTrades,
            BlockInfo,
            DexPrice,
            MevBlocks,
            AddressToProtocolInfo,
            TokenDecimals,
            SubGraphs,
            TxTraces,
            Builder,
            AddressMeta,
            SearcherEOAs,
            SearcherContracts,
            InitializedState,
            PoolCreationBlocks = &self.key,
            &self.value
        );

        Ok(())
    }
}
