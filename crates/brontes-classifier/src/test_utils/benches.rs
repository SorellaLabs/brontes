use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, BlockTracesWithHeaderAnd, TraceLoader, TraceLoaderError,
    TxTracesWithHeaderAnd,
};
use brontes_database::{
    libmdbx::LibmdbxReadWriter, AddressToProtocolInfo, AddressToProtocolInfoData,
};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    db::address_to_protocol_info::ProtocolInfo, normalized_actions::Action,
    structured_trace::TraceActions, tree::BlockTree,
};
use criterion::{black_box, Criterion};
use reth_db::DatabaseError;
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{ActionCollection, Classifier, ProtocolClassifier};

pub struct ClassifierBenchUtils {
    trace_loader:          TraceLoader,
    classifier:            Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,
    rt:                    tokio::runtime::Runtime,
    _dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}

impl Default for ClassifierBenchUtils {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassifierBenchUtils {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let trace_loader = rt.block_on(TraceLoader::new());
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());
        Self { classifier, trace_loader, _dex_pricing_receiver: rx, rt }
    }

    pub fn bench_tx_tree_building(
        &self,
        bench_name: &str,
        tx_hash: TxHash,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        let TxTracesWithHeaderAnd { trace, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx_hash))?;

        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter_batched(
                || (vec![trace.clone()], header.clone()),
                |(trace, header)| async move {
                    black_box(self.classifier.build_block_tree(trace, header, true).await)
                },
                criterion::BatchSize::NumIterations(1),
            );
        });

        Ok(())
    }

    pub fn bench_txes_tree_building(
        &self,
        bench_name: &str,
        tx_hashes: Vec<TxHash>,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_traces_with_header(tx_hashes))?
            .remove(0);

        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter_batched(
                || (traces.clone(), header.clone()),
                |(trace, header)| async move {
                    black_box(self.classifier.build_block_tree(trace, header, true).await)
                },
                criterion::BatchSize::NumIterations(1),
            );
        });

        Ok(())
    }

    pub fn bench_block_tree_building(
        &self,
        bench_name: &str,
        block: u64,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_block_traces_with_header(block))?;

        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter_batched(
                || (traces.clone(), header.clone()),
                |(trace, header)| async move {
                    black_box(self.classifier.build_block_tree(trace, header, true).await)
                },
                criterion::BatchSize::NumIterations(1),
            );
        });
        Ok(())
    }

    #[allow(clippy::unit_arg)]
    pub fn bench_tree_operations_tx(
        &self,
        bench_name: &str,
        tx: TxHash,
        c: &mut Criterion,
        bench_fn: impl Fn(Arc<BlockTree<Action>>),
    ) -> Result<(), ClassifierBenchError> {
        let TxTracesWithHeaderAnd { trace, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx))?;

        let tree = self
            .rt
            .block_on(self.classifier.build_block_tree(vec![trace], header, true));
        let tree = Arc::new(tree);

        c.bench_function(bench_name, move |b| b.iter(|| black_box(bench_fn(tree.clone()))));

        Ok(())
    }

    #[allow(clippy::unit_arg)]
    pub fn bench_tree_operations(
        &self,
        bench_name: &str,
        block: u64,
        c: &mut Criterion,
        bench_fn: impl Fn(Arc<BlockTree<Action>>),
    ) -> Result<(), ClassifierBenchError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_block_traces_with_header(block))?;
        let tree = self
            .rt
            .block_on(self.classifier.build_block_tree(traces, header, true));
        let tree = Arc::new(tree);

        c.bench_function(bench_name, move |b| b.iter(|| black_box(bench_fn(tree.clone()))));

        Ok(())
    }

    pub fn bench_protocol_classification(
        &self,
        bench_name: &str,
        iters: usize,
        tx: TxHash,
        protocol: ProtocolInfo,
        protocol_address: Address,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        // write protocol to libmdbx
        self.trace_loader
            .libmdbx
            .db
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&[
                AddressToProtocolInfoData { key: protocol_address, value: protocol },
            ])?;

        let TxTracesWithHeaderAnd { trace, block, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx))?;

        let trace = trace
            .trace
            .into_iter()
            .find(|t| t.get_to_address() == protocol_address)
            .ok_or_else(|| ClassifierBenchError::ProtocolClassifierError(protocol_address))?;

        let dispatcher = ProtocolClassifier::default();

        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                let call_info = trace.get_callframe_info();

                for _ in 0..=iters {
                    black_box(dispatcher.dispatch(
                        call_info.clone(),
                        self.trace_loader.libmdbx,
                        block,
                        0,
                        self.trace_loader.get_provider(),
                    ));
                }
            })
        });

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ClassifierBenchError {
    #[error(transparent)]
    TraceLoaderError(#[from] TraceLoaderError),
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error("couldn't find trace for address: {0:?}")]
    DiscoveryError(Address),
    #[error("couldn't find parent node for created pool {0:?}")]
    ProtocolDiscoveryError(Address),
    #[error("couldn't find trace that matched {0:?}")]
    ProtocolClassifierError(Address),
}
