use brontes_database::{libmdbx::Libmdbx, Tables};
use clap::Parser;
use eyre::Ok;

#[derive(Debug, Parser)]
pub struct Clear {
    /// Tables to clear
    #[arg(
        long,
        short,
        value_delimiter = ',',
        default_value = "CexPrice,DexPrice,CexTrades,BlockInfo,InitializedState,MevBlocks,\
                         TokenDecimals,AddressToProtocolInfo,PoolCreationBlocks,Builder,\
                         AddressMeta,SearcherEOAs,SearcherContracts,SubGraphs,TxTraces"
    )]
    pub tables: Vec<Tables>,
}

impl Clear {
    pub async fn execute(self, brontes_db_endpoint: String) -> eyre::Result<()> {
        let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

        macro_rules! clear_table {
    ($table:expr, $($tables:ident),+) => {
        match $table {
            $(
                Tables::$tables => {
                            db
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
                AddressMeta,
                SearcherEOAs,
                SearcherContracts,
                TxTraces
            )
        });

        #[cfg(feature = "local-reth")]
        if !self.tables.contains(&Tables::InitializedState)
            && [Tables::CexPrice, Tables::BlockInfo, Tables::DexPrice]
                .iter()
                .any(|table| self.tables.contains(table))
        {
            db.clear_table::<brontes_database::libmdbx::tables::InitializedState>()?;
        }

        #[cfg(not(feature = "local-reth"))]
        if !self.tables.contains(&Tables::InitializedState)
            && [Tables::CexPrice, Tables::BlockInfo, Tables::DexPrice, Tables::TxTraces]
                .iter()
                .any(|table| self.tables.contains(table))
        {
            db.clear_table::<brontes_database::libmdbx::tables::InitializedState>()?;
        }

        Ok(())
    }
}
