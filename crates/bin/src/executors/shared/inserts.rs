use std::sync::Arc;

use brontes_database::libmdbx::{LibmdbxReader, LibmdbxWriter};
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    db::{metadata::Metadata, searcher::SearcherInfo},
    mev::{Bundle, MevBlock},
    normalized_actions::Actions,
    tree::BlockTree,
};
use tracing::{error, info};

pub async fn process_results<DB: LibmdbxWriter + LibmdbxReader>(
    db: &DB,
    inspectors: &[&dyn Inspector],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) {
    let ComposerResults { block_details, mev_details, possible_mev_txes: _ } =
        compose_mev_results(inspectors, tree, metadata.clone()).await;

    if let Err(e) = db.write_dex_quotes(metadata.block_num.clone(), metadata.dex_quotes.clone()) {
        tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
    }

    insert_mev_results(db, block_details, mev_details);
}

fn insert_mev_results<DB: LibmdbxWriter + LibmdbxReader>(
    database: &DB,
    block_details: MevBlock,
    mev_details: Vec<Bundle>,
) {
    info!(
        target: "brontes",
        "block details\n {}",
        block_details.to_string()
    );

    output_mev_and_update_searcher_info(database, block_details.block_number, &mev_details);

    // Attempt to save the MEV block details
    if let Err(e) = database.save_mev_blocks(block_details.block_number, block_details, mev_details)
    {
        error!("Failed to insert classified data into libmdbx: {:?}", e);
    }
}

fn output_mev_and_update_searcher_info<DB: LibmdbxWriter + LibmdbxReader>(
    database: &DB,
    block_number: u64,
    mev_details: &Vec<Bundle>,
) {
    for mev in mev_details {
        info!(
            target: "brontes",
            "mev details\n {}",
            mev.to_string()
        );

        // Attempt to fetch existing searcher info
        let result = database.try_fetch_searcher_info(mev.header.eoa);

        let mut searcher_info = match result {
            Ok(info) => info,
            Err(_) => SearcherInfo::default(),
        };

        // Update the searcher info with the current MEV details
        searcher_info.pnl += mev.header.profit_usd;
        searcher_info.total_bribed += mev.header.bribe_usd;
        if !searcher_info.mev.contains(&mev.header.mev_type) {
            searcher_info.mev.push(mev.header.mev_type.clone());
        }
        searcher_info.last_active = block_number;

        if let Err(e) = database.write_searcher_info(mev.header.eoa, searcher_info) {
            error!("Failed to update searcher info in the database: {:?}", e);
        }
    }
}
