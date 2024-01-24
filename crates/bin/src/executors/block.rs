use std::{
    cmp::max,
    pin::Pin,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::load_missing_decimals,
};
use brontes_database::libmdbx::{
    tables::{DexPrice, Metadata as MetadataTable, MevBlocks},
    types::{dex_price::make_filter_key_range, mev_block::MevBlocksData, LibmdbxData},
    Libmdbx, LibmdbxReader, LibmdbxWriter,
};
use brontes_inspect::{
    composer::{compose_mev_results, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{BundleData, BundleHeader, MevBlock},
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        cex::{CexPriceMap, CexQuote},
        dex::DexQuotes,
        metadata::{MetadataCombined, MetadataInner, MetadataNoDex},
        mev_block::MevBlockWithClassified,
    },
    extra_processing::Pair,
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{task::waker, Future, FutureExt};
use tracing::{debug, error, info, trace};

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (MetadataCombined, BlockTree<Actions>)> + Send + 'a>>;

pub struct BlockInspector<'inspector, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    block_number: u64,

    parser:            &'inspector Parser<'inspector, T, DB>,
    classifier:        &'inspector Classifier<'inspector, T, DB>,
    database:          &'inspector DB,
    inspectors:        &'inspector [&'inspector Box<dyn Inspector>],
    composer_future:   Option<Pin<Box<dyn Future<Output = ComposerResults> + Send + 'inspector>>>,
    // pending future data
    classifier_future: Option<CollectionFut<'inspector>>,
    // pending insertion data
    // insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader>
    BlockInspector<'inspector, T, DB>
{
    pub fn new(
        parser: &'inspector Parser<'inspector, T, DB>,
        database: &'inspector DB,
        classifier: &'inspector Classifier<'_, T, DB>,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>],
        block_number: u64,
    ) -> Self {
        Self {
            block_number,
            inspectors,
            parser,
            database,
            classifier,
            composer_future: None,
            classifier_future: None,
        }
    }

    fn start_collection(&mut self) {
        trace!(target:"brontes", block_number = self.block_number, "starting collection of data");
        let parser_fut = self.parser.execute(self.block_number);
        let labeller_fut = self.database.get_metadata(self.block_number);

        let classifier_fut = Box::pin(async {
            let (traces, header) = parser_fut.await.unwrap().unwrap();
            debug!("Got {} traces + header", traces.len());
            let block_number = header.number;
            let (extra_data, tree) = self.classifier.build_block_tree(traces, header).await;

            load_missing_decimals(
                self.parser.get_tracer(),
                self.database,
                block_number,
                extra_data.tokens_decimal_fill,
            )
            .await;

            let meta = labeller_fut.unwrap();

            (meta, tree)
        });

        self.classifier_future = Some(classifier_fut);
    }

    fn on_inspectors_finish(&mut self, results: (MevBlock, Vec<(BundleHeader, BundleData)>)) {
        trace!(
            block_number = self.block_number,
            "inserting the collected results \n {:#?}",
            results
        );
        if self
            .database
            .save_mev_blocks(self.block_number, results.0, results.1)
            .is_err()
        {
            error!("failed to insert classified mev to Libmdbx");
        }
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((meta_data, tree)) => {
                    self.composer_future = Some(Box::pin(compose_mev_results(
                        self.inspectors,
                        tree.into(),
                        meta_data.into(),
                    )));
                }
                Poll::Pending => {
                    self.classifier_future = Some(collection_fut);
                    return
                }
            }
        }

        if let Some(mut inner) = self.composer_future.take() {
            if let Poll::Ready(ComposerResults { block_details, mev_details, .. }) =
                inner.poll_unpin(cx)
            {
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
                self.on_inspectors_finish((block_details, mev_details));
            } else {
                self.composer_future = Some(inner);
            }
        }
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Future for BlockInspector<'_, T, DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If the classifier_future is None (not started yet), start the collection
        // phase
        if self.classifier_future.is_none() && self.composer_future.is_none() {
            self.start_collection();
        }

        self.progress_futures(cx);

        // Decide when to finish the BlockInspector's future.
        // Finish when both classifier and insertion futures are done.
        if self.classifier_future.is_none() && self.composer_future.is_none() {
            info!(
                target:"brontes",
                block_number = self.block_number, "finished inspecting block");

            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
