use alloy_primitives::Address;
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
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
}
