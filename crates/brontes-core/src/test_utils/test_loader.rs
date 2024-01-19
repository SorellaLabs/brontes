use std::env;

use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_metrics::PoirotMetricEvents;
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use futures::future::join_all;
use reth_primitives::{Header, B256};
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use super::init_trace_parser;
use crate::{decoding::parser::TraceParser, init_tracing};

/// Functionality to load all state needed for any testing requirments
pub struct TestLoader {
    libmdbx:          &'static Libmdbx,
    tracing_provider: TraceParser<'static, Box<dyn TracingProvider>>,
    _metrics:         UnboundedReceiver<PoirotMetricEvents>,
}

impl TestLoader {
    pub fn new() -> Self {
        let _ = dotenv::dotenv();
        init_tracing();

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None).unwrap()));

        let (a, b) = unbounded_channel();
        let tracing_provider = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10);
        Self { libmdbx, tracing_provider, _metrics: b }
    }

    async fn trace_block(&self, block: u64) -> Result<(Vec<TxTrace>, Header), TestLoaderError> {
        self.tracing_provider
            .execute_block(block)
            .await
            .ok_or_else(|| TestLoaderError::BlockTraceError(block))
    }

    async fn get_metadata(&self, block: u64) -> Result<Metadata, TestLoaderError> {
        self.libmdbx
            .get_metadata(block)
            .map_err(|_| TestLoaderError::NoMetadataFound(block))
    }

    pub async fn get_block_traces_with_header(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<()>, TestLoaderError> {
        let (traces, header) = self.trace_block(block).await?;
        Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
    }

    pub async fn get_block_traces_with_header_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<()>>, TestLoaderError> {
        join_all(
            (start_block..=end_block)
                .into_iter()
                .map(|block| async move {
                    let (traces, header) = self.trace_block(block).await?;
                    Ok(BlockTracesWithHeaderAnd { traces, header, block, other: () })
                }),
        )
        .await
        .into_iter()
        .collect()
    }

    pub async fn get_block_traces_with_header_and_metadata(
        &self,
        block: u64,
    ) -> Result<BlockTracesWithHeaderAnd<Metadata>, TestLoaderError> {
        let (traces, header) = self.trace_block(block).await?;
        let metadata = self.get_metadata(block).await?;

        Ok(BlockTracesWithHeaderAnd { block, traces, header, other: metadata })
    }

    pub async fn get_block_traces_with_header_and_metadata_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<BlockTracesWithHeaderAnd<Metadata>>, TestLoaderError> {
        join_all(
            (start_block..=end_block)
                .into_iter()
                .map(|block| async move {
                    let (traces, header) = self.trace_block(block).await?;
                    let metadata = self.get_metadata(block).await?;
                    Ok(BlockTracesWithHeaderAnd { traces, header, block, other: metadata })
                }),
        )
        .await
        .into_iter()
        .collect()
    }

    pub async fn get_tx_traces_with_header(
        &self,
        tx_hash: B256,
    ) -> Result<TxTracesWithHeaderAnd<()>, TestLoaderError> {
        let (block, tx_idx) = self
            .tracing_provider
            .get_tracer()
            .block_and_tx_index(tx_hash)
            .await?;
        let (traces, header) = self.trace_block(block).await?;
        let trace = traces[tx_idx];

        Ok(TxTracesWithHeaderAnd {
            block,
            tx_hash,
            trace,
            header,
            other: ()
        })
    }

    pub async fn get_tx_traces_with_header_and_metadata(
        &self,
        tx_hash: B256,
    ) -> Result<TxTracesWithHeaderAnd<Metadata>, TestLoaderError> {
        let (block, tx_idx) = self
            .tracing_provider
            .get_tracer()
            .block_and_tx_index(tx_hash)
            .await?;
        let (traces, header) = self.trace_block(block).await?;
        let metadata = self.get_metadata(block).await?;
        let trace = traces[tx_idx];

        Ok(TxTracesWithHeaderAnd {
            block,
            tx_hash,
            trace,
            header,
            other: metadata
        })
    }
}

#[derive(Debug, Error)]
pub enum TestLoaderError {
    #[error("no metadata found in libmdbx for block: {0}")]
    NoMetadataFound(u64),
    #[error("failed to trace block: {0}")]
    BlockTraceError(u64),
    #[error(transparent)]
    ProviderError(#[from] ProviderError),
}

pub struct TxTracesWithHeaderAnd<T> {
    pub block:   u64,
    pub tx_hash: B256,
    pub trace:   TxTrace,
    pub header:  Header,
    pub other:   T,
}

pub struct BlockTracesWithHeaderAnd<T> {
    pub block:  u64,
    pub traces: Vec<TxTrace>,
    pub header: Header,
    pub other:  T,
}
