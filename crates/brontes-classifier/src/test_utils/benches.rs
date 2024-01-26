use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, BlockTracesWithHeaderAnd, TraceLoader, TraceLoaderError,
    TxTracesWithHeaderAnd,
};
use brontes_database::{
    libmdbx::{types::address_to_protocol::AddressToProtocolData, LibmdbxReadWriter},
    AddressToProtocol,
};
use brontes_pricing::{types::DexPriceMsg, Protocol};
use brontes_types::{normalized_actions::Actions, structured_trace::TraceActions, tree::BlockTree};
use criterion::{black_box, Criterion};
use reth_db::DatabaseError;
use reth_rpc_types::trace::parity::Action;
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{
    ActionCollection, Classifier, DiscoveryProtocols, FactoryDecoderDispatch,
    ProtocolClassifications,
};

pub struct ClassifierBenchUtils {
    trace_loader:          TraceLoader,
    classifier:            Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,
    rt:                    tokio::runtime::Runtime,
    _dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
}
impl ClassifierBenchUtils {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let trace_loader = TraceLoader::new_with_rt(rt.handle().clone());
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
                    black_box(self.classifier.build_block_tree(trace, header))
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
                    black_box(self.classifier.build_block_tree(trace, header))
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
                    black_box(self.classifier.build_block_tree(trace, header))
                },
                criterion::BatchSize::NumIterations(1),
            );
        });
        Ok(())
    }

    pub fn bench_tree_operations_tx(
        &self,
        bench_name: &str,
        tx: TxHash,
        c: &mut Criterion,
        bench_fn: impl Fn(Arc<BlockTree<Actions>>),
    ) -> Result<(), ClassifierBenchError> {
        let TxTracesWithHeaderAnd { trace, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx))?;

        let tree = self
            .rt
            .block_on(self.classifier.build_block_tree(vec![trace], header));
        let tree = Arc::new(tree);

        c.bench_function(bench_name, move |b| b.iter(|| black_box(bench_fn(tree.clone()))));

        Ok(())
    }

    pub fn bench_tree_operations(
        &self,
        bench_name: &str,
        block: u64,
        c: &mut Criterion,
        bench_fn: impl Fn(Arc<BlockTree<Actions>>),
    ) -> Result<(), ClassifierBenchError> {
        let BlockTracesWithHeaderAnd { traces, header, .. } = self
            .rt
            .block_on(self.trace_loader.get_block_traces_with_header(block))?;
        let tree = self
            .rt
            .block_on(self.classifier.build_block_tree(traces, header));
        let tree = Arc::new(tree);

        c.bench_function(bench_name, move |b| b.iter(|| black_box(bench_fn(tree.clone()))));

        Ok(())
    }

    pub fn bench_protocol_discovery(
        &self,
        bench_name: &str,
        iters: usize,
        tx: TxHash,
        created_pool: Address,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        let TxTracesWithHeaderAnd { trace, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx))?;

        let found_trace = trace
            .trace
            .iter()
            .filter(|t| t.is_create())
            .find(|t| t.get_create_output() == created_pool)
            .ok_or_else(|| ClassifierBenchError::DiscoveryError(created_pool))?;

        let mut trace_addr = found_trace.get_trace_address();

        if trace_addr.len() > 1 {
            trace_addr.pop().unwrap();
        } else {
            return Err(ClassifierBenchError::ProtocolDiscoveryError(created_pool))
        };

        let p_trace = trace
            .trace
            .iter()
            .find(|f| f.get_trace_address() == trace_addr)
            .ok_or_else(|| ClassifierBenchError::ProtocolDiscoveryError(created_pool))?;

        let Action::Call(call) = &p_trace.trace.action else { panic!() };

        c.bench_function(bench_name, move |b| {
            b.to_async(&self.rt).iter(|| async move {
                let from_address = found_trace.get_from_addr();
                let created_addr = found_trace.get_create_output();
                let dispatcher = DiscoveryProtocols::default();
                let call_data = call.input.clone();
                let tracer = self.trace_loader.get_provider();

                for _ in 0..=iters {
                    black_box(dispatcher.dispatch(
                        tracer.clone(),
                        from_address,
                        created_addr,
                        call_data.clone(),
                    ))
                    .await;
                }
            })
        });

        Ok(())
    }

    pub fn bench_protocol_classification(
        &self,
        bench_name: &str,
        iters: usize,
        tx: TxHash,
        protocol: Protocol,
        protocol_address: Address,
        c: &mut Criterion,
    ) -> Result<(), ClassifierBenchError> {
        // write protocol to libmdbx
        self.trace_loader
            .libmdbx
            .0
            .write_table::<AddressToProtocol, AddressToProtocolData>(&vec![
                AddressToProtocolData {
                    address:         protocol_address,
                    classifier_name: protocol,
                },
            ])?;

        let TxTracesWithHeaderAnd { trace, block, .. } = self
            .rt
            .block_on(self.trace_loader.get_tx_trace_with_header(tx))?;

        let trace = trace
            .trace
            .into_iter()
            .find(|t| t.get_to_address() == protocol_address)
            .ok_or_else(|| ClassifierBenchError::ProtocolClassificationError(protocol_address))?;

        let dispatcher = ProtocolClassifications::default();

        c.bench_function(bench_name, move |b| {
            b.iter(|| {
                let from_address = trace.get_from_addr();
                let target_address = trace.get_to_address();

                let call_data = trace.get_calldata();
                let return_bytes = trace.get_return_calldata();

                for _ in 0..=iters {
                    black_box(dispatcher.dispatch(
                        0,
                        call_data.clone(),
                        return_bytes.clone(),
                        from_address,
                        target_address,
                        trace.msg_sender,
                        &trace.logs,
                        self.trace_loader.libmdbx,
                        block,
                        0,
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
    ProtocolClassificationError(Address),
}
