use std::{
    pin::Pin,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::MissingDecimals,
};
use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_inspect::{
    composer::{Composer, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{Future, FutureExt};
use tracing::{debug, error, info, trace};
type CollectionFut<'a> = Pin<Box<dyn Future<Output = (Metadata, BlockTree<Actions>)> + Send + 'a>>;

pub struct BlockInspector<'inspector, const N: usize, T: TracingProvider> {
    block_number: u64,

    parser:            &'inspector Parser<'inspector, T>,
    classifier:        &'inspector Classifier<'inspector>,
    database:          &'inspector Libmdbx,
    inspectors:        &'inspector [&'inspector Box<dyn Inspector>; N],
    composer_future:   Option<Pin<Box<dyn Future<Output = ComposerResults> + Send + 'inspector>>>,
    // pending future data
    classifier_future: Option<CollectionFut<'inspector>>,
    // pending insertion data
    // insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, const N: usize, T: TracingProvider> BlockInspector<'inspector, N, T> {
    pub fn new(
        parser: &'inspector Parser<'inspector, T>,
        database: &'inspector Libmdbx,
        classifier: &'inspector Classifier,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>; N],
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
            let (extra_data, mut tree) = self.classifier.build_block_tree(traces, header);

            MissingDecimals::new(
                self.parser.get_tracer(),
                self.database,
                extra_data.tokens_decimal_fill,
            )
            .await;

            let meta = labeller_fut.unwrap();
            tree.eth_price = meta.eth_prices.clone();

            (meta, tree)
        });

        self.classifier_future = Some(classifier_fut);
    }

    fn on_inspectors_finish(
        &mut self,
        results: (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>),
    ) {
        trace!(
            block_number = self.block_number,
            "inserting the collected results \n {:#?}",
            results
        );

        if self
            .database
            .insert_classified_data(results.0, results.1)
            .is_err()
        {
            error!("failed to insert classified mev to Libmdbx");
        }
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((meta_data, tree)) => {
                    self.composer_future = Some(Box::pin(Composer::new(
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
            if let Poll::Ready(data) = inner.poll_unpin(cx) {
                info!(
                    target:"brontes",
                    "Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
                     ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
                     {}\n- Cumulative MEV Priority Fee Paid: {}\n- Builder Address: {:?}\n- \
                     Builder ETH Profit: {}\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer \
                     Fee Recipient: {:?}\n- Proposer MEV Reward: {:?}\n- Proposer Finalized \
                     Profit (USD): {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n",
                    data.0.block_number,
                    data.0.mev_count,
                    data.0.finalized_eth_price,
                    data.0.cumulative_gas_used,
                    data.0.cumulative_gas_paid,
                    data.0.total_bribe,
                    data.0.cumulative_mev_priority_fee_paid,
                    data.0.builder_address,
                    data.0.builder_eth_profit,
                    data.0.builder_finalized_profit_usd,
                    data.0
                        .proposer_fee_recipient
                        .map_or(Address::ZERO.to_string(), |v| format!("{:?}", v)),
                    data.0
                        .proposer_mev_reward
                        .map_or("None".to_string(), |v| v.to_string()),
                    data.0
                        .proposer_finalized_profit_usd
                        .map_or("None".to_string(), |v| format!("{:.2}", v)),
                    data.0.cumulative_mev_finalized_profit_usd
                );
                self.on_inspectors_finish(data);
            } else {
                self.composer_future = Some(inner);
            }
        }
    }
}

impl<const N: usize, T: TracingProvider> Future for BlockInspector<'_, N, T> {
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
