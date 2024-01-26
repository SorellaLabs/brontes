use std::collections::HashMap;

use alloy_primitives::{Address, TxHash};
use brontes_classifier::Classifier;
use brontes_core::test_utils::*;
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{normalized_actions::Actions, traits::TracingProvider, tree::BlockTree};
use thiserror::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

type PricingResult<T> = Result<T, PricingTestError>;

pub struct PricingTestUtils {
    tracer:        TraceLoader,
    quote_address: Address,
}

impl PricingTestUtils {
    pub fn new(quote_address: Address) -> Self {
        let tracer = TraceLoader::new();
        Self { tracer, quote_address }
    }

    async fn init_dex_pricer(
        &self,
        block: u64,
        end_block: Option<u64>,
        rx: UnboundedReceiver<DexPriceMsg>,
    ) -> Result<BrontesBatchPricer<Box<dyn TracingProvider>>, PricingTestError> {
        let pairs = self
            .tracer
            .libmdbx
            .protocols_created_before(block)
            .map_err(|_| PricingTestError::LibmdbxError)?;

        let pair_graph = GraphManager::init_from_db_state(
            pairs,
            HashMap::default(),
            Box::new(|_, _| None),
            Box::new(|_, _, _| {}),
        );

        let created_pools = if let Some(end_block) = end_block {
            self.tracer
                .libmdbx
                .protocols_created_range(block + 1, end_block)
                .unwrap()
                .into_iter()
                .flat_map(|(_, pools)| {
                    pools
                        .into_iter()
                        .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                        .collect::<Vec<_>>()
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        Ok(BrontesBatchPricer::new(
            self.quote_address,
            pair_graph,
            rx,
            self.tracer.get_provider(),
            block,
            created_pools,
        ))
    }

    /// traces the block and classifies it sending all messages to the batch
    /// pricer.
    pub async fn setup_dex_pricer_for_block(
        &self,
        block: u64,
    ) -> PricingResult<(BrontesBatchPricer<Box<dyn TracingProvider>>, BlockTree<Actions>)> {
        let BlockTracesWithHeaderAnd { traces, header, .. } =
            self.tracer.get_block_traces_with_header(block).await?;

        let (tx, rx) = unbounded_channel();
        let pricer = self.init_dex_pricer(block, None, rx).await?;

        let classifier = Classifier::new(self.tracer.libmdbx, tx, self.tracer.get_provider());
        let tree = classifier.build_block_tree(traces, header).await;
        Ok((pricer, tree))
    }

    pub async fn setup_dex_pricer_for_tx(
        &self,
        tx_hash: TxHash,
    ) -> Result<BrontesBatchPricer<Box<dyn TracingProvider>>, PricingTestError> {
        let TxTracesWithHeaderAnd { trace, header, block, .. } =
            self.tracer.get_tx_trace_with_header(tx_hash).await?;
        let (tx, rx) = unbounded_channel();

        let classifier = Classifier::new(self.tracer.libmdbx, tx, self.tracer.get_provider());
        let mut pricer = self.init_dex_pricer(block, None, rx).await?;

        let tree = classifier.build_block_tree(vec![trace], header).await;

        Ok(pricer)
    }
}

#[derive(Debug, Error)]
pub enum PricingTestError {
    #[error(transparent)]
    TraceError(#[from] TraceLoaderError),
    #[error("libmdbx error")]
    LibmdbxError,
}
