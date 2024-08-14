use brontes_database::{libmdbx::Libmdbx, InitializedState, InitializedStateData, Tables};
use brontes_types::db::initialized_state::{
    CEX_QUOTES_FLAG, CEX_TRADES_FLAG, DEX_PRICE_FLAG, META_FLAG, TRACE_FLAG,
};
use clap::Parser;

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
    pub tables:                  Vec<Tables>,
    /// Mark metadata as uninitialized in the initialized state table
    #[arg(long, default_value = "false")]
    pub clear_metadata_flags:    bool,
    /// Mark cex data as uninitialized in the initialized state table
    #[arg(long, default_value = "false")]
    pub clear_cex_flags:         bool,
    /// Mark tx traces as uninitialized in the initialized state table
    #[arg(long, default_value = "false")]
    pub clear_tx_traces_flags:   bool,
    /// Mark dex pricing as uninitialized in the initialized state table
    #[arg(long, default_value = "false")]
    pub clear_dex_pricing_flags: bool,
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

        // self.tables.iter().for_each(|table| {
        //     clear_table!(
        //         table,
        //         CexPrice,
        //         CexTrades,
        //         InitializedState,
        //         BlockInfo,
        //         DexPrice,
        //         MevBlocks,
        //         TokenDecimals,
        //         AddressToProtocolInfo,
        //         PoolCreationBlocks,
        //         Builder,
        //         AddressMeta,
        //         SearcherEOAs,
        //         SearcherContracts,
        //         TxTraces
        //     )
        // });
        if self.clear_cex_flags {
            db.view_db(|tx| {
                let mut cur = tx.new_cursor::<InitializedState>()?;
                let walker = cur.walk_range(..)?;
                let mut updated_res = Vec::new();
                for mut item in walker.flatten() {
                    item.1.apply_reset_key(CEX_QUOTES_FLAG);
                    updated_res.push(InitializedStateData::new(item.0, item.1));
                }
                db.write_table(&updated_res)?;
                Ok(())
            })
            .unwrap();
        }

        // if self.clear_cex_flags
        //     || self.clear_tx_traces_flags
        //     || self.clear_metadata_flags
        //     || self.clear_dex_pricing_flags
        // {
        //     db.view_db(|tx| {
        //         let mut cur = tx.new_cursor::<InitializedState>()?;
        //         let walker = cur.walk_range(..)?;
        //         let mut updated_res = Vec::new();
        //
        //         for item in walker.flatten() {
        //             let mut key = item.1;
        //             if self.clear_dex_pricing_flags {
        //                 key.apply_reset_key(DEX_PRICE_FLAG);
        //             }
        //             if self.clear_metadata_flags {
        //                 key.apply_reset_key(META_FLAG);
        //             }
        //             if self.clear_tx_traces_flags {
        //                 key.apply_reset_key(TRACE_FLAG);
        //             }
        //             if self.clear_cex_flags {
        //                 key.apply_reset_key(CEX_QUOTES_FLAG);
        //                 key.apply_reset_key(CEX_TRADES_FLAG);
        //             }
        //
        //             key.apply_reset_key(SKIP_FLAG);
        //             updated_res.push(InitializedStateData::new(item.0, item.1));
        //         }
        //         db.write_table(&updated_res)?;
        //         Ok(())
        //     })?;
        // }

        Ok(())
    }
}
