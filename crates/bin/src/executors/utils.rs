use std::sync::Arc;

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxWriter;
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, MevBlock, PossibleMev},
    db::metadata::MetadataCombined,
    normalized_actions::Actions,
    tree::BlockTree,
};
use colored::Colorize;
use tracing::{error, info};

pub async fn process_results<DB: LibmdbxWriter>(
    db: &DB,
    inspectors: &[&Box<dyn Inspector>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<MetadataCombined>,
) -> Vec<PossibleMev> {
    let ComposerResults { block_details, mev_details, possible_mev_txes } =
        compose_mev_results(inspectors, tree, metadata.clone()).await;

    if let Err(e) = db.write_dex_quotes(metadata.block_num.clone(), metadata.dex_quotes.clone()) {
        tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
    }

    insert_mev_results(db, block_details, mev_details);
    possible_mev_txes
}

fn insert_mev_results<DB: LibmdbxWriter>(
    database: &DB,
    block_details: MevBlock,
    mev_details: Vec<(BundleHeader, BundleData)>,
) {
    let mev_summary = block_details
        .possible_mev
        .iter()
        .map(|possible_mev| {
            let eth_paid = possible_mev.gas_details.gas_paid() as f64 * 1e-18;
            let tx_url = format!("https://etherscan.io/tx/{:?}", possible_mev.tx_hash);
            format!(
                "{} paid {} ETH for inclusion\nEtherscan link: {}\n{}",
                format!("Tx number {}", possible_mev.tx_idx).blue(),
                eth_paid.to_string().green(),
                tx_url.underline().blue(),
                possible_mev.triggers
            )
        })
        .fold(String::new(), |acc, line| acc + &line + "\n");

    info!(
        target:"brontes",
        "\n Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
         ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
         {}\n- Cumulative MEV Priority Fee Paid: {} ETH \n- Builder Address: {:?}\n- Builder \
         ETH Profit: {} ETH\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer Fee \
         Recipient: {:?}\n- Proposer MEV Reward: {:?} ETH \n- Proposer Finalized Profit (USD): \
        {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n- Possibly Missed Mev:\n{}",
        block_details.block_number,
        block_details.mev_count.to_string().bold().red(),
        block_details.eth_price,
        block_details.cumulative_gas_used as f64 * 1e-18,
        block_details.cumulative_gas_paid as f64 * 1e-18,
        block_details.total_bribe as f64 * 1e-18,
        block_details.cumulative_mev_priority_fee_paid as f64 * 1e-18,
        block_details.builder_address,
        block_details.builder_eth_profit,
        block_details.builder_profit_usd,
        block_details
            .proposer_fee_recipient
            .unwrap_or(Address::ZERO),
        block_details
            .proposer_mev_reward
            .map_or(0.0, |v| v as f64 * 1e-18),
        block_details
            .proposer_profit_usd
            .map_or("None".to_string(), |v| format!("{:.2}", v)),
        block_details.cumulative_mev_profit_usd,
        mev_summary
    );

    info!("{:#?}", mev_details);

    if database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .is_err()
    {
        error!("failed to insert classified data into libmdx");
    }
}
