use alloy_primitives::Address;
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_pricing::{types::ProtocolState, LoadState};
use brontes_types::{pair::Pair, Protocol};
use criterion::{black_box, Criterion};

pub struct BrontesPricingBencher {
    inner:       ClassifierTestUtils,
    quote_asset: Address,
    rt:          tokio::runtime::Runtime,
}
impl BrontesPricingBencher {
    pub fn new(quote_asset: Address) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let inner = rt.block_on(ClassifierTestUtils::new());

        Self { inner, quote_asset, rt }
    }

    pub fn bench_pricing_block(
        &self,
        bench_name: &str,
        block_number: u64,
        c: &mut Criterion,
    ) -> Result<(), ClassifierTestUtilsError> {
        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async move {
                black_box(
                    self.inner
                        .build_block_tree_with_pricing(block_number, self.quote_asset, vec![])
                        .await,
                )
            })
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
                        )
                        .await,
                )
            })
        });
        Ok(())
    }
}
