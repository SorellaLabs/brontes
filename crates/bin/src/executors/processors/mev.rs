use std::sync::Arc;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    db::metadata::Metadata,
    execute_on,
    mev::{Bundle, MevBlock, MevType},
    normalized_actions::Actions,
    tree::BlockTree,
};
use tracing::info;

use crate::Processor;

#[derive(Debug, Clone, Copy)]
pub struct MevProcessor;

impl Processor for MevProcessor {
    type InspectType = Vec<Bundle>;

    async fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) {
        let ComposerResults { block_details, mev_details, possible_mev_txes: _ } =
            execute_on!(target = inspect, compose_mev_results(inspectors, tree, metadata.clone()));

        if let Err(e) = db
            .write_dex_quotes(metadata.block_num, metadata.dex_quotes.clone())
            .await
        {
            tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
        }

        insert_mev_results(db, block_details, mev_details).await;
    }
}

async fn insert_mev_results<DB: DBWriter + LibmdbxReader>(
    database: &DB,
    block_details: MevBlock,
    mev_details: Vec<Bundle>,
) {
    info!(
        target: "brontes",
        "block details\n {}",
        block_details.to_string()
    );

    let block_number = block_details.block_number;
    output_mev_and_update_searcher_info(database, &mev_details).await;

    // Attempt to save the MEV block details
    if let Err(e) = database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .await
    {
        tracing::error!(
            "Failed to insert classified data into libmdbx: {:?} at block: {}",
            e,
            block_number
        );
    }
}
async fn output_mev_and_update_searcher_info<DB: DBWriter + LibmdbxReader>(
    database: &DB,
    mev_details: &Vec<Bundle>,
) {
    for mev in mev_details {
        info!(
            target: "brontes",
            "mev details\n {}",
            mev.to_string()
        );

        if mev.header.mev_type == MevType::Unknown || mev.header.mev_type == MevType::SearcherTx {
            continue;
        }

        let (eoa_info, contract_info) = database
            .try_fetch_searcher_info(mev.header.eoa, mev.header.mev_contract)
            .expect("Failed to fetch searcher info from the database");

        let mut eoa_info = eoa_info.unwrap_or_default();
        let mut contract_info = contract_info.unwrap_or_default();

        if !eoa_info.mev.contains(&mev.header.mev_type) {
            eoa_info.mev.push(mev.header.mev_type);
        }

        if !contract_info.mev.contains(&mev.header.mev_type) {
            contract_info.mev.push(mev.header.mev_type);
        }

        if let Err(e) = database
            .write_searcher_info(
                mev.header.eoa,
                mev.header.mev_contract,
                eoa_info,
                Some(contract_info),
            )
            .await
        {
            tracing::error!("Failed to update searcher info in the database: {:?}", e);
        }
    }
}
