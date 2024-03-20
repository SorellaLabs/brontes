use std::sync::Arc;

use alloy_primitives::Address;
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_pricing::{types::ProtocolState, LoadState};
use brontes_types::{pair::Pair, Protocol};
use criterion::{black_box, Criterion};
use futures::StreamExt;

pub struct BrontesPricingBencher {
    inner:       Arc<ClassifierTestUtils>,
    quote_asset: Address,
    rt:          tokio::runtime::Runtime,
}
impl BrontesPricingBencher {
    pub fn new(quote_asset: Address) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let inner = Arc::new(rt.block_on(ClassifierTestUtils::new()));

        Self { inner, quote_asset, rt }
    }

    pub fn bench_pricing_block(
        &self,
        bench_name: &str,
        block_number: u64,
        c: &mut Criterion,
    ) -> Result<(), ClassifierTestUtilsError> {
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter_batched(
                || {
                    let inner = self.inner.clone();
                    let quote_asset = self.quote_asset;
                    // annoying but otherwise blockin in blockin
                    std::thread::spawn(move || {
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(inner.clone().setup_pricing_for_bench(
                                block_number,
                                quote_asset,
                                vec![],
                            ))
                            .unwrap()
                    })
                    .join()
                    .unwrap()
                },
                |(mut data, _tx)| async move { black_box(data.next().await) },
                criterion::BatchSize::LargeInput,
            )
        });

        Ok(())
    }

    pub fn bench_pool_state_price(
        &self,
        bench_name: &str,
        pool: Address,
        block_number: u64,
        pool_pair: Pair,
        protocol: Protocol,
        c: &mut Criterion,
    ) -> Result<(), ClassifierTestUtilsError> {
        let state = self
            .rt
            .block_on(protocol.try_load_state(
                pool,
                self.inner.get_tracing_provider(),
                block_number,
                pool_pair,
                pool_pair,
                pool_pair,
            ))
            .unwrap()
            .2;

        let pair_1 = pool_pair.0;
        c.bench_function(bench_name, move |b| b.iter(|| black_box(state.price(pair_1).unwrap())));

        Ok(())
    }

    pub fn bench_pool_state_loads(
        &self,
        bench_name: &str,
        pool: Address,
        block_number: u64,
        pool_pair: Pair,
        protocol: Protocol,
        c: &mut Criterion,
    ) -> Result<(), ClassifierTestUtilsError> {
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async move {
                black_box(
                    protocol
                        .try_load_state(
                            pool,
                            self.inner.get_tracing_provider(),
                            block_number,
                            pool_pair,
                            pool_pair,
                            pool_pair,
                        )
                        .await,
                )
            })
        });
        Ok(())
    }
}
