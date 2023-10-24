use sorella_db_databases::{ClickhouseClient, *};

use crate::classified_mev::{
    AtomicBackrun, CexDex, ClassifiedMev, JitLiquidity, JitLiquiditySandwich, Liquidation,
    MevBlock, Sandwich,
};

fn spawn_db() -> ClickhouseClient {
    dotenv::dotenv().ok();
    ClickhouseClient::default()
}

#[tokio::test]
async fn test_db_mev_block() {
    let test_block = MevBlock::default();

    let db = spawn_db();

    db.insert_one(test_block.clone(), MEV_BLOCKS_TABLE)
        .await
        .unwrap();

    db.execute(&format!(
        "DELETE FROM {MEV_BLOCKS_TABLE} where block_hash = '{:?}' and block_number = {}",
        test_block.block_hash, test_block.block_number
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_classified_mev() {
    let test_mev = ClassifiedMev::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), CLASSIFIED_MEV_TABLE)
        .await
        .unwrap();

    db.execute(&format!(
        "DELETE FROM {CLASSIFIED_MEV_TABLE} where tx_hash = '{:?}' and block_number = {}",
        test_mev.tx_hash, test_mev.block_number
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_sandwhich() {
    let test_mev = Sandwich::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), SANWHICH_TABLE)
        .await
        .unwrap();

    db.execute(&format!(
        "DELETE FROM {SANWHICH_TABLE} where front_run_tx_hash = '{:?}' and backrun_tx_hash = 
         '{:?}'",
        test_mev.frontrun_tx_hash, test_mev.backrun_tx_hash
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_jit_sandwhich() {
    let test_mev = JitLiquiditySandwich::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), JIT_SANDWHICH_TABLE)
        .await
        .unwrap();

    db.execute(&format!(
        "DELETE FROM {JIT_SANDWHICH_TABLE} where frontrun_tx_hash = '{:?}' and burn_tx_hash = \
         '{:?}'",
        test_mev.frontrun_tx_hash, test_mev.burn_tx_hash
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_jit() {
    let test_mev = JitLiquidity::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), JIT_TABLE).await.unwrap();

    db.execute(&format!(
        "DELETE FROM {JIT_TABLE} where mint_tx_hash = '{:?}' and burn_tx_hash = '{:?}'",
        test_mev.mint_tx_hash, test_mev.burn_tx_hash
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_liquidation() {
    let test_mev = Liquidation::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), LIQUIDATIONS_TABLE)
        .await
        .unwrap();

    db.execute(&format!(
        "DELETE FROM {LIQUIDATIONS_TABLE} where liquidation_tx_hash = '{:?}' and trigger = '{:?}'",
        test_mev.liquidation_tx_hash, test_mev.trigger
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_atomic_backrun() {
    let test_mev = AtomicBackrun::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), BACKRUN_TABLE)
        .await
        .unwrap();

    db.execute(&format!("DELETE FROM {BACKRUN_TABLE} where tx_hash = '{:?}'", test_mev.tx_hash))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_db_cex_dex() {
    let test_mev = CexDex::default();

    let db = spawn_db();

    db.insert_one(test_mev.clone(), CEX_DEX_TABLE)
        .await
        .unwrap();

    db.execute(&format!("DELETE FROM {CEX_DEX_TABLE} where tx_hash = '{:?}'", test_mev.tx_hash))
        .await
        .unwrap();
}
