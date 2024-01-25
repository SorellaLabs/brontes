use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use brontes_classifier::test_utils::ClassifierTestUtils;
use brontes_types::db::metadata::MetadataCombined;
use criterion::{black_box, Criterion};

use super::InspectorTestUtilsError;
use crate::{composer::compose_mev_results, Inspectors};

pub struct InspectorBenchUtils {
    classifier_inspector: ClassifierTestUtils,
    quote_address:        Address,
    rt:                   tokio::runtime::Runtime,
}
impl InspectorBenchUtils {
    pub fn new(quote_address: Address) -> Self {
        let classifier_inspector = ClassifierTestUtils::new();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        Self { classifier_inspector, quote_address, rt }
    }

    pub fn bench_inspectors_block(
        &self,
        bench_name: &str,
        block: u64,
        iters: usize,
        inspectors: Vec<Inspectors>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspectors = inspectors
            .into_iter()
            .map(|i| i.init_inspector(self.quote_address, self.classifier_inspector.libmdbx))
            .collect::<Vec<_>>();

        let (tree, prices) = self.rt.block_on(
            self.classifier_inspector
                .build_tree_block_with_pricing(block, self.quote_address),
        )?;

        let mut metadata = self
            .rt
            .block_on(self.classifier_inspector.get_metadata(block))?;
        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async {
                for _ in 0..=iters {
                    for inspector in &inspectors {
                        black_box(inspector.process_tree(tree.clone(), metadata.clone()).await);
                    }
                }
            });
        });

        Ok(())
    }

    pub fn bench_inspector_txes(
        &self,
        bench_name: &str,
        tx_hashes: Vec<TxHash>,
        iters: usize,
        inspector: Inspectors,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector =
            inspector.init_inspector(self.quote_address, self.classifier_inspector.libmdbx);

        let mut trees = self.rt.block_on(
            self.classifier_inspector
                .build_tree_txes_with_pricing(tx_hashes, self.quote_address),
        )?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|(t, _)| t.header.number).collect(),
            ))
        }

        let (tree, prices) = trees.remove(0);

        let mut metadata = self
            .rt
            .block_on(self.classifier_inspector.get_metadata(tree.header.number))?;
        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async {
                for _ in 0..=iters {
                    black_box(inspector.process_tree(tree.clone(), metadata.clone()).await);
                }
            });
        });

        Ok(())
    }

    pub fn bench_inspector_txes_with_meta(
        &self,
        bench_name: &str,
        tx_hashes: Vec<TxHash>,
        metadata: MetadataCombined,
        iters: usize,
        inspector: Inspectors,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector =
            inspector.init_inspector(self.quote_address, self.classifier_inspector.libmdbx);

        let mut trees = self
            .rt
            .block_on(self.classifier_inspector.build_tree_txes(tx_hashes))?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|t| t.header.number).collect(),
            ))
        }

        let tree = trees.remove(0);

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async {
                for _ in 0..=iters {
                    black_box(inspector.process_tree(tree.clone(), metadata.clone()).await);
                }
            });
        });

        Ok(())
    }

    pub fn bench_composer(
        &self,
        bench_name: &str,
        tx_hashes: Vec<TxHash>,
        iters: usize,
        inspectors: Vec<Inspectors>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspectors = inspectors
            .into_iter()
            .map(|i| i.init_inspector(self.quote_address, self.classifier_inspector.libmdbx))
            .collect::<Vec<_>>();
        let mut trees = self
            .rt
            .block_on(self.classifier_inspector.build_tree_txes(tx_hashes))?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|t| t.header.number).collect(),
            ))
        }

        let mut metadata = self
            .rt
            .block_on(self.classifier_inspector.get_metadata(tree.header.number))?;
        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async {
                for _ in 0..=iters {
                    black_box(
                        compose_mev_results(inspectors.as_slice(), tree.clone(), metadata.clone())
                            .await,
                    );
                }
            });
        });

        Ok(())
    }
}
