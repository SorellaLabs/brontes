pub mod bundle;
pub use bundle::*;
pub mod sandwich;
pub use sandwich::*;
pub mod jit;
pub use jit::*;
pub mod backrun;
pub use backrun::*;
pub mod cex_dex;
pub use cex_dex::*;
pub mod liquidation;
pub use liquidation::*;
pub mod jit_sandwich;
pub use jit_sandwich::*;
pub mod block;
pub use block::*;

#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use sorella_db_databases::{
        clickhouse::db::ClickhouseClient,
        tables::{DatabaseTables, FromDatabaseTables},
        Database,
    };

    use super::*;

    fn spawn_db() -> ClickhouseClient {
        ClickhouseClient::default()
    }

    // #[tokio::test]
    // async fn test_db_mev_block() {
    //     let test_block = MevBlock::default();
    //
    //     let db: ClickhouseClient = spawn_db();
    //
    //     db.insert_one(&test_block, DatabaseTables::MevBlocks)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where block_hash = ? and block_number = ?",
    //         db.to_table_string(DatabaseTables::MevBlocks)
    //     );
    //     db.execute_remote(
    //         &delete_query,
    //         &(format!("{:?}", test_block.block_hash),
    // test_block.block_number),     )
    //     .await
    //     .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_classified_mev() {
    //     let test_mev = BundleHeader::default();
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::BundleHeader)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = &format!(
    //         "DELETE FROM {} where tx_hash = ? and block_number = ?",
    //         db.to_table_string(DatabaseTables::BundleHeader)
    //     );
    //
    //     db.execute_remote(
    //         &delete_query,
    //         &(format!("{:?}", test_mev.tx_hash), test_mev.block_number),
    //     )
    //     .await
    //     .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_sandwich() {
    //     let test_mev = Sandwich::default();
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::Sandwich)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where frontrun_tx_hash = ? and backrun_tx_hash =
    // ?",         db.to_table_string(DatabaseTables::Sandwich)
    //     );
    //     db.execute_remote(
    //         &delete_query,
    //         &(
    //             format!("{:?}", test_mev.frontrun_tx_hash),
    //             format!("{:?}", test_mev.backrun_tx_hash),
    //         ),
    //     )
    //     .await
    //     .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_jit_sandwhich() {
    //     let test_mev = JitLiquiditySandwich::default();
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::JitSandwich)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where frontrun_tx_hash = ? and backrun_tx_hash =
    // ?",         db.to_table_string(DatabaseTables::JitSandwich)
    //     );
    //
    //     db.execute_remote(
    //         &delete_query,
    //         &(
    //             format!("{:?}", test_mev.frontrun_tx_hash),
    //             format!("{:?}", test_mev.backrun_tx_hash),
    //         ),
    //     )
    //     .await
    //     .unwrap();
    // }

    // #[tokio::test]
    // async fn test_db_jit() {
    //     let mut test_mev: JitLiquidity = JitLiquidity::default();
    //     test_mev.frontrun_mints.push(Default::default());
    //     test_mev.backrun_burn_gas_details.coinbase_transfer = None;
    //     test_mev.backrun_burns.iter_mut().for_each(|burn| {
    //         burn.token = vec![
    //
    // Address::from_str("0xb17548c7b510427baac4e267bea62e800b247173").unwrap(),
    //
    // Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
    //         ];
    //         burn.from = Default::default();
    //         burn.to = Default::default();
    //         burn.recipient = Default::default();
    //         burn.trace_index = Default::default();
    //         burn.amount = vec![Default::default()];
    //     });
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::Jit).await.unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where frontrun_mint_tx_hash = ? and
    // backrun_burn_tx_hash = ?",         db.
    // to_table_string(DatabaseTables::Jit)     );
    //
    //     db.execute_remote(
    //         &delete_query,
    //         &(
    //             format!("{:?}", test_mev.frontrun_mint_tx_hash),
    //             format!("{:?}", test_mev.backrun_burn_tx_hash),
    //         ),
    //     )
    //     .await
    //     .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_liquidation() {
    //     let test_mev = Liquidation::default();
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::Liquidations)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where liquidation_tx_hash = ?",
    //         db.to_table_string(DatabaseTables::Liquidations)
    //     );
    //     db.execute_remote(&delete_query, &(format!("{:?}",
    // test_mev.liquidation_tx_hash)))         .await
    //         .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_atomic_backrun() {
    //     let test_mev = AtomicArb::default();
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::AtomicArb)
    //         .await
    //         .unwrap();
    //
    //     let delete_query = format!(
    //         "DELETE FROM {} where tx_hash = ?",
    //         db.to_table_string(DatabaseTables::AtomicArb)
    //     );
    //     db.execute_remote(&delete_query, &(format!("{:?}",
    // test_mev.tx_hash)))         .await
    //         .unwrap();
    // }
    //
    // #[tokio::test]
    // async fn test_db_cex_dex() {
    //     let test_mev = CexDex::default();
    //
    //     let db = spawn_db();
    //
    //     db.insert_one(&test_mev, DatabaseTables::CexDex)
    //         .await
    //         .unwrap();
    //
    //     let delete_query =
    //         format!("DELETE FROM {} where tx_hash = ?",
    // db.to_table_string(DatabaseTables::CexDex));     db.execute_remote(&
    // delete_query, &(format!("{:?}", test_mev.tx_hash)))         .await
    //         .unwrap();
    // }
}
