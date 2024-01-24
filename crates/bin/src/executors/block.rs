use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::load_missing_decimals,
};
use brontes_database::libmdbx::{LibmdbxReader, LibmdbxWriter};
use brontes_inspect::Inspector;
use brontes_types::{db::metadata::MetadataCombined, normalized_actions::Actions, tree::BlockTree};
use eyre::eyre;
use futures::{Future, FutureExt};
use tracing::{debug, error, info, trace};

use super::utils::process_results;

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = eyre::Result<(MetadataCombined, BlockTree<Actions>)>> + Send + 'a>>;

/// The block inspector executes for a single block. The executor will fail
/// when metadata is missing
pub struct BlockInspector<'inspector, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    block_number: u64,

    parser:            &'inspector Parser<'inspector, T, DB>,
    classifier:        &'inspector Classifier<'inspector, T, DB>,
    database:          &'inspector DB,
    inspectors:        &'inspector [&'inspector Box<dyn Inspector>],
    composer_future:   Option<Pin<Box<dyn Future<Output = ()> + Send + 'inspector>>>,
    // pending future data
    classifier_future: Option<CollectionFut<'inspector>>,
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
            let (traces, header) = parser_fut.await?.ok_or_else(|| eyre!("parser failed"))?;
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

            let meta = labeller_fut?;

            Ok((meta, tree))
        });

        self.classifier_future = Some(classifier_fut);
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready(Ok((meta_data, tree))) => {
                    self.composer_future = Some(Box::pin(
                        process_results(
                            self.database,
                            self.inspectors,
                            tree.into(),
                            meta_data.into(),
                        )
                        .map(|_| ()),
                    ))
                }
                Poll::Ready(Err(e)) => {
                    error!(error=%e, "block inspector errored");
                }
                Poll::Pending => {
                    self.classifier_future = Some(collection_fut);
                    return
                }
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
