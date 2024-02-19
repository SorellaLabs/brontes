use std::sync::Arc;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
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

pub async fn process_results<DB: DBWriter + LibmdbxReader>(
    db: &DB,
    // clickhouse-db (feature)
    inspectors: &[&dyn Inspector<Result = Vec<Bundle>>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<Metadata>,
) {
    let ComposerResults {
        block_details,
        mev_details,
        possible_mev_txes: _,
    } = compose_mev_results(inspectors, tree, metadata.clone()).await;

    // insert the value to the respective table:
    // clickhouse_db.insert_many::<T>(Vec<D>).await.unwrap()
    // where T is the clickhouse table name
    // and D is the clickhouse table's data type

    if let Err(e) = db
        .write_dex_quotes(metadata.block_num, metadata.dex_quotes.clone())
        .await
    {
        tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
    }

    insert_mev_results(db, block_details, mev_details).await;
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

    output_mev_and_update_searcher_info(database, &mev_details).await;

    // Attempt to save the MEV block details
    if let Err(e) = database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .await
    {
        error!("Failed to insert classified data into libmdbx: {:?}", e);
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

        // If the contract is verified or is a know protocol, we only update the EOA info
        if database
            .try_fetch_address_metadata(mev.header.mev_contract)
            .expect("Failed to fetch address metadata from the database")
            .unwrap_or_default()
            .is_verified()
            || database
                .get_protocol_details(mev.header.mev_contract)
                .is_ok()
        {
            if let Err(e) = database
                .write_searcher_eoa_info(mev.header.eoa, eoa_info)
                .await
            {
                error!("Failed to update searcher info in the database: {:?}", e);
            }
        } else {
            if let Err(e) = database
                .write_searcher_info(
                    mev.header.eoa,
                    mev.header.mev_contract,
                    eoa_info,
                    contract_info,
                )
                .await
            {
                error!("Failed to update searcher info in the database: {:?}", e);
            }
        }
    }
}
