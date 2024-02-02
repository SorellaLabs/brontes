use brontes_classifier::Classifier;
use brontes_core::{decoding::Parser, LibmdbxReader, LibmdbxWriter};
use brontes_types::{
    db::metadata::MetadataCombined, normalized_actions::Actions, traits::TracingProvider, BlockTree,
};
use eyre::eyre;
use tracing::info;

use super::metadata::MetadataFetcher;

pub async fn collect_all_state<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter>(
    block: u64,
    db: &'static DB,
    metadata_fetcher: MetadataFetcher<T, DB>,
    parser: &'static Parser<'static, T, DB>,
    classifier: &'static Classifier<'static, T, DB>,
) -> eyre::Result<(MetadataFetcher<T, DB>, BlockTree<Actions>, MetadataCombined)> {
    let (traces, header) = parser
        .execute(block)
        .await?
        .ok_or_else(|| eyre!("no traces for block {block}"))?;

    info!("Got {} traces + header", traces.len());
    let tree = classifier.build_block_tree(traces, header).await;
    let (tree, meta) = metadata_fetcher
        .load_metadata_for_tree(block, tree, db)
        .await?;

    Ok((metadata_fetcher, meta, tree))
}
