use std::sync::Arc;

use alloy_primitives::{Address, B256};
use brontes_database::libmdbx::LibmdbxWriter;
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, MevBlock},
    db::metadata::MetadataCombined,
    normalized_actions::Actions,
    tree::BlockTree,
};
use tracing::{error, info};

pub async fn process_results<DB: LibmdbxWriter>(
    db: &DB,
    inspectors: &[&Box<dyn Inspector>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<MetadataCombined>,
) -> Vec<(B256, u128)> {
    let ComposerResults { block_details, mev_details, possibly_missed_arbs } =
        compose_mev_results(inspectors, tree, metadata.clone()).await;

    if let Err(e) = db.write_dex_quotes(metadata.block_num.clone(), metadata.dex_quotes.clone()) {
        tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
    }

    insert_mev_results(db, block_details, mev_details);
    possibly_missed_arbs
}

fn insert_mev_results<DB: LibmdbxWriter>(
    database: &DB,
    block_details: MevBlock,
    mev_details: Vec<(BundleHeader, BundleData)>,
) {
    info!(
        target:"brontes",
        "Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
         ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
         {}\n- Cumulative MEV Priority Fee Paid: {}\n- Builder Address: {:?}\n- Builder \
         ETH Profit: {}\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer Fee \
         Recipient: {:?}\n- Proposer MEV Reward: {:?}\n- Proposer Finalized Profit (USD): \
        {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n- Possibly Missed Mev:\n{}",
        block_details.block_number,
        block_details.mev_count,
        block_details.finalized_eth_price,
        block_details.cumulative_gas_used,
        block_details.cumulative_gas_paid,
        block_details.total_bribe,
        block_details.cumulative_mev_priority_fee_paid,
        block_details.builder_address,
        block_details.builder_eth_profit,
        block_details.builder_finalized_profit_usd,
        block_details
            .proposer_fee_recipient
            .unwrap_or(Address::ZERO),
        block_details
            .proposer_mev_reward
            .map_or("None".to_string(), |v| v.to_string()),
        block_details
            .proposer_finalized_profit_usd
            .map_or("None".to_string(), |v| format!("{:.2}", v)),
        block_details.cumulative_mev_finalized_profit_usd,
    block_details
        .possible_missed_arbs
        .iter()
        .map(|arb| format!("https://etherscan.io/tx/{arb:?}"))
        .fold(String::new(), |acc, arb| acc + &arb + "\n")
    );
    info!("{:#?}", mev_details);

    if database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .is_err()
    {
        error!("failed to insert classified data into libmdx");
    }
}
