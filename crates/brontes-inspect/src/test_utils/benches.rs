use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use brontes_classifier::test_utils::ClassifierTestUtils;
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

    pub async fn bench_inspectors_block(
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

        let (tree, prices) = self
            .classifier_inspector
            .build_tree_block_with_pricing(block, self.quote_address)
            .await?;

        let mut metadata = self.classifier_inspector.get_metadata(block).await?;
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

    pub async fn bench_inspector_tx(
        &self,
        bench_name: &str,
        tx_hash: TxHash,
        iters: usize,
        inspector: Inspectors,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector =
            inspector.init_inspector(self.quote_address, self.classifier_inspector.libmdbx);

        let (tree, prices) = self
            .classifier_inspector
            .build_tree_tx_with_pricing(tx_hash, self.quote_address)
            .await?;

        let mut metadata = self
            .classifier_inspector
            .get_metadata(tree.header.number)
            .await?;
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

    pub async fn bench_composer(
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

        let (tree, prices) = self
            .classifier_inspector
            .build_tree_block_with_pricing(block, self.quote_address)
            .await?;

        let mut metadata = self
            .classifier_inspector
            .get_metadata(tree.header.number)
            .await?;
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
