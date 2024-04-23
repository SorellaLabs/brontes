use std::{
    sync::{atomic::Ordering::SeqCst, Arc, Mutex},
    time::{Duration, Instant},
};

use alloy_primitives::Address;
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_pricing::{types::ProtocolState, LoadState};
use brontes_types::{pair::Pair, Protocol};
use criterion::{black_box, BenchmarkId, Criterion};
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

    /// benches price generation after n amount of blocks
    #[allow(clippy::await_holding_lock)]
    pub fn bench_pricing_post_init(
        &self,
        bench_name: &str,
        start_block_number: u64,
        bench_past_n_blocks: u64,
        c: &mut Criterion,
    ) -> Result<(), ClassifierTestUtilsError> {
        // get upto date
        let (dex_pricer, tx, ctr) = self
            .rt
            .block_on(self.inner.setup_pricing_for_bench_post_init(
                start_block_number,
                bench_past_n_blocks - 1,
                self.quote_asset,
                vec![],
            ))
            .unwrap();

        // snapshot current state
        let (reg, ver, state) = dex_pricer.snapshot_graph_state();
        let dex_pricer = Mutex::new(dex_pricer);

        c.bench_with_input(
            BenchmarkId::new("benching_dex_pricing_post_init", bench_name),
            &(dex_pricer, tx, ctr, reg, ver, state),
            |b, (dex_pricer, tx, ctr, reg, ver, state)| {
                b.to_async(&self.rt).iter_custom(|iters| {
                    let inner = self.inner.clone();
                    async move {
                        let mut dex_pricer = dex_pricer.lock().unwrap();
                        // snapshot current state
                        ctr.store(false, SeqCst);

                        let mut total_dur = Duration::ZERO;
                        tracing::info!("starting post init bench");
                        for _ in 0..iters {
                            // setup traces for block
                            inner
                                .send_traces_for_block(
                                    start_block_number + bench_past_n_blocks,
                                    tx.clone(),
                                )
                                .await
                                .unwrap();

                            ctr.store(true, SeqCst);
                            let start = Instant::now();
                            black_box(dex_pricer.next().await);
                            total_dur += start.elapsed();

                            // reset for next block
                            ctr.store(false, SeqCst);

                            dex_pricer.set_state(reg.clone(), ver.clone(), state.clone());
                            *dex_pricer.completed_block() -= 1;
                        }
                        total_dur
                    }
                })
            },
        );

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
                brontes_pricing::types::PairWithFirstPoolHop::from_pair_gt(pool_pair, pool_pair),
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
                            brontes_pricing::types::PairWithFirstPoolHop::from_pair_gt(
                                pool_pair, pool_pair,
                            ),
                        )
                        .await,
                )
            })
        });
        Ok(())
    }
}
