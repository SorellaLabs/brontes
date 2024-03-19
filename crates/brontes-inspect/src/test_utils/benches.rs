use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use brontes_classifier::test_utils::ClassifierTestUtils;
use brontes_types::db::{cex::CexExchange, metadata::Metadata};
use criterion::{black_box, Criterion};

use super::InspectorTestUtilsError;
use crate::{composer::compose_mev_results, Inspectors};

pub struct InspectorBenchUtils {
    classifier_inspector: ClassifierTestUtils,
    quote_address:        Address,
    pub rt:               tokio::runtime::Runtime,
}

impl InspectorBenchUtils {
    pub fn new(quote_address: Address) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        let classifier_inspector = rt.block_on(ClassifierTestUtils::new());
        Self { classifier_inspector, quote_address, rt }
    }

    pub fn bench_inspectors_block(
        &self,
        bench_name: &str,
        block: u64,
        iters: usize,
        inspectors: Vec<Inspectors>,
        needed_tokens: Vec<Address>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspectors = inspectors
            .into_iter()
            .map(|i| {
                i.init_mev_inspector(
                    self.quote_address,
                    self.classifier_inspector.libmdbx,
                    &[CexExchange::Binance],
                )
            })
            .collect::<Vec<_>>();

        let (tree, prices) =
            self.rt
                .block_on(self.classifier_inspector.build_block_tree_with_pricing(
                    block,
                    self.quote_address,
                    needed_tokens,
                ))?;

        let mut metadata = self
            .rt
            .block_on(self.classifier_inspector.get_metadata(block, false))
            .unwrap_or_default();

        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                for _ in 0..=iters {
                    for inspector in &inspectors {
                        black_box(inspector.process_tree(tree.clone(), metadata.clone()));
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
        inspector_type: Inspectors,
        needed_tokens: Vec<Address>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector = inspector_type.init_mev_inspector(
            self.quote_address,
            self.classifier_inspector.libmdbx,
            &[CexExchange::Binance],
        );

        let mut trees =
            self.rt
                .block_on(self.classifier_inspector.build_tree_txes_with_pricing(
                    tx_hashes,
                    self.quote_address,
                    needed_tokens,
                ))?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|(t, _)| t.header.number).collect(),
            ))
        }

        let (tree, prices) = trees.remove(0);

        let mut metadata = self.rt.block_on(async move {
            let res = self
                .classifier_inspector
                .get_metadata(tree.header.number, false)
                .await;

            #[cfg(not(feature = "cex-dex-markout"))]
            let cmp = Inspectors::CexDex;
            #[cfg(feature = "cex-dex-markout")]
            let cmp = Inspectors::CexDexMarkout;

            if inspector_type == cmp {
                res
            } else {
                Ok(res.unwrap_or_else(|_| Metadata::default()))
            }
        })?;

        metadata.dex_quotes = Some(prices);

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                for _ in 0..=iters {
                    black_box(inspector.process_tree(tree.clone(), metadata.clone()));
                }
            });
        });

        Ok(())
    }

    pub fn bench_inspector_block(
        &self,
        bench_name: &str,
        block: u64,
        iters: usize,
        inspector_type: Inspectors,
        needed_tokens: Vec<Address>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector = inspector_type.init_mev_inspector(
            self.quote_address,
            self.classifier_inspector.libmdbx,
            &[CexExchange::Binance],
        );

        let (tree, prices) =
            self.rt
                .block_on(self.classifier_inspector.build_block_tree_with_pricing(
                    block,
                    self.quote_address,
                    needed_tokens,
                ))?;

        let mut metadata = self.rt.block_on(async move {
            let res = self
                .classifier_inspector
                .get_metadata(tree.header.number, false)
                .await;

            #[cfg(not(feature = "cex-dex-markout"))]
            let cmp = Inspectors::CexDex;
            #[cfg(feature = "cex-dex-markout")]
            let cmp = Inspectors::CexDexMarkout;

            if inspector_type == cmp {
                res
            } else {
                Ok(res.unwrap_or_else(|_| Metadata::default()))
            }
        })?;
        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                for _ in 0..=iters {
                    black_box(inspector.process_tree(tree.clone(), metadata.clone()));
                }
            });
        });

        Ok(())
    }

    pub fn bench_inspector_txes_with_meta(
        &self,
        bench_name: &str,
        tx_hashes: Vec<TxHash>,
        metadata: Metadata,
        iters: usize,
        inspector: Inspectors,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspector = inspector.init_mev_inspector(
            self.quote_address,
            self.classifier_inspector.libmdbx,
            &[CexExchange::Binance],
        );

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
            b.iter(|| {
                for _ in 0..=iters {
                    black_box(inspector.process_tree(tree.clone(), metadata.clone()));
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
        needed_tokens: Vec<Address>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspectors = inspectors
            .into_iter()
            .map(|i| {
                i.init_mev_inspector(
                    self.quote_address,
                    self.classifier_inspector.libmdbx,
                    &[CexExchange::Binance],
                )
            })
            .collect::<Vec<_>>();

        let mut trees =
            self.rt
                .block_on(self.classifier_inspector.build_tree_txes_with_pricing(
                    tx_hashes,
                    self.quote_address,
                    needed_tokens,
                ))?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|(t, _)| t.header.number).collect(),
            ))
        }
        let (tree, prices) = trees.remove(0);

        let mut metadata = self.rt.block_on(
            self.classifier_inspector
                .get_metadata(tree.header.number, false),
        )?;
        metadata.dex_quotes = Some(prices);

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                for _ in 0..=iters {
                    black_box(compose_mev_results(
                        inspectors.as_slice(),
                        tree.clone(),
                        metadata.clone(),
                    ));
                }
            });
        });

        Ok(())
    }

    pub fn bench_composer_block(
        &self,
        bench_name: &str,
        block: u64,
        iters: usize,
        inspectors: Vec<Inspectors>,
        needed_tokens: Vec<Address>,
        c: &mut Criterion,
    ) -> Result<(), InspectorTestUtilsError> {
        let inspectors = inspectors
            .into_iter()
            .map(|i| {
                i.init_mev_inspector(
                    self.quote_address,
                    self.classifier_inspector.libmdbx,
                    &[CexExchange::Binance],
                )
            })
            .collect::<Vec<_>>();

        let (tree, prices) =
            self.rt
                .block_on(self.classifier_inspector.build_block_tree_with_pricing(
                    block,
                    self.quote_address,
                    needed_tokens,
                ))?;

        let mut metadata = self.rt.block_on(
            self.classifier_inspector
                .get_metadata(tree.header.number, false),
        )?;
        metadata.dex_quotes = prices;

        let (tree, metadata) = (Arc::new(tree), Arc::new(metadata));
        c.bench_function(bench_name, move |b| {
            b.iter(|| async {
                for _ in 0..=iters {
                    black_box(compose_mev_results(
                        inspectors.as_slice(),
                        tree.clone(),
                        metadata.clone(),
                    ));
                }
            });
        });

        Ok(())
    }
}
