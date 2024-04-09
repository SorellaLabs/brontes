#[cfg(feature = "local-clickhouse")]
use std::sync::Arc;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::{
    composer::{run_block_inspection, ComposerResults},
    Inspector,
};
#[cfg(feature = "local-clickhouse")]
use brontes_types::frontend_prunes::{
    remove_burn_transfers, remove_collect_transfers, remove_mint_transfers, remove_swap_transfers,
};
#[cfg(feature = "local-clickhouse")]
use brontes_types::normalized_actions::Action;
#[cfg(feature = "local-clickhouse")]
use brontes_types::tree::BlockTree;
use brontes_types::{
    db::block_analysis::BlockAnalysis,
    execute_on,
    mev::{
        events::{Action, TuiEvents},
        Bundle, MevBlock, MevType,
    },
    normalized_actions::Actions,
    tree::BlockTree,
};
use clap::Parser;
//tui related
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::{
    cli::{Args, Commands},
    Processor,
};

#[derive(Debug, Clone, Copy)]
pub struct MevProcessor;

impl Processor for MevProcessor {
    type InspectType = Vec<Bundle>;

    async fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
        tui_tx: Option<UnboundedSender<Action>>,
    ) {
        let ComposerResults { block_details, mev_details, possible_mev_txes: _ } = execute_on!(
            target = inspect,
            compose_mev_results(inspectors, tree.clone(), metadata.clone(), tui_tx)
        );

        if let Err(e) = db
            .write_dex_quotes(metadata.block_num, metadata.dex_quotes.clone())
            .await
        {
            tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
        }

        #[cfg(feature = "local-clickhouse")]
        insert_tree(db, tree.clone(), metadata.block_num).await;

        let opt = Args::parse();
        match opt.command {
            Commands::Run(command) => {
                if !command.cli_only {
                    let _ = tui_tx
                        .clone()
                        .unwrap()
                        .send(Action::Tui(TuiEvents::MevBlockMetricReceived(block_details.clone())))
                        .map_err(|e| {
                            use tracing::info;
                            info!("Failed to send: {}", e);
                        });

                    let _ = tui_tx
                        .unwrap()
                        .send(Action::Tui(TuiEvents::MevBundleEventReceived(mev_details.clone())))
                        .map_err(|e| {
                            use tracing::info;
                            info!("Failed to send: {}", e);
                        });
                }
            }
            _ => {}
        }

        insert_mev_results(db, block_details, mev_details).await;
    }
}

#[cfg(feature = "local-clickhouse")]
async fn insert_tree<DB: DBWriter + LibmdbxReader>(
    db: &DB,
    mut tree_owned: BlockTree<Action>,
    block_num: u64,
) {
    remove_swap_transfers(&mut tree_owned);
    remove_mint_transfers(&mut tree_owned);
    remove_burn_transfers(&mut tree_owned);
    remove_collect_transfers(&mut tree_owned);

    if let Err(e) = db.insert_tree(tree_owned).await {
        tracing::error!(err=%e, %block_num, "failed to insert tree into db");
    }
}

async fn insert_mev_results<DB: DBWriter + LibmdbxReader>(
    database: &'static DB,
    block_details: MevBlock,
    mev_details: Vec<Bundle>,
    analysis: BlockAnalysis,
) {
    debug!(
        target: "brontes::results",
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
    if let Err(e) = database.write_block_analysis(analysis).await {
        tracing::error!(
            "Failed to insert block analysis data into db: {:?} at block: {}",
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
        debug!(
            target: "brontes::results",
            "mev details\n {}",
            mev.to_string()
        );

        if mev.header.mev_type == MevType::Unknown || mev.header.mev_type == MevType::SearcherTx {
            continue
        }

        let (eoa_info, contract_info) = database
            .try_fetch_searcher_info(mev.header.eoa, mev.header.mev_contract)
            .expect("Failed to fetch searcher info from the database");

        let mut eoa_info = eoa_info.unwrap_or_default();
        let mut contract_info = contract_info.unwrap_or_default();

        eoa_info.update_with_bundle(&mev.header);
        contract_info.update_with_bundle(&mev.header);

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
