use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxWriter;
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    db::metadata::Metadata,
    mev::{Bundle, MevBlock},
    normalized_actions::Actions,
    tree::BlockTree,
};
use tracing::{error, info};

pub async fn process_results<DB: LibmdbxWriter>(
    db: &DB,
    inspectors: &[&Box<dyn Inspector>],
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

fn insert_mev_results<DB: LibmdbxWriter>(
    database: &DB,
    block_details: MevBlock,
    mev_details: Vec<Bundle>,
) {
    info!(
        target: "brontes",
        "block details\n {}",
        block_details.to_string()
    );

    for mev in &mev_details {
        info!(
            target: "brontes",
            "mev details\n {}",
            mev.to_string()
        );
    }

    if database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .is_err()
    {
        error!("failed to insert classified data into libmdx");
    }
}
