use alloy_primitives::{Address, TxHash};
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_core::TraceLoaderError;
use brontes_database::Metadata;
use brontes_types::classified_mev::{MevType, SpecificMev};
use thiserror::Error;

use crate::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};

/// This run config is used for a single opportunity test.
/// it supports multiple hashes for sandwiches
#[derive(Debug, Clone)]
pub struct InspectorTxRunConfig {
    pub metadata_override:   Option<Metadata>,
    pub mev_tx_hashes:       Option<Vec<TxHash>>,
    pub expected_profit_usd: Option<f64>,
    pub expected_gas_usd:    Option<f64>,
    pub expected_mev_type:   Option<MevType>,
}

#[derive(Debug, Clone)]
pub struct ComposerRunConfig {
    pub inspectors: Option<Vec<MevType>>,

    pub metadata_override: Option<Metadata>,
    pub mev_tx_hashes:     Option<Vec<TxHash>>,
    pub result_hash:       Option<TxHash>,
    pub expected_mev_type: Option<MevType>,
}

/// Inspector Specific testing functionality
pub struct InspectorTestUtils {
    classifier_inspector:  ClassifierTestUtils,
    quote_address:         Address,
    max_result_difference: f64,
}

impl InspectorTestUtils {
    pub fn new(quote_address: Address, max_result_difference: f64) -> Self {
        let classifier_inspector = ClassifierTestUtils::new();
        Self { classifier_inspector, quote_address, max_result_difference }
    }

    pub async fn run_inspector<F>(
        &self,
        config: InspectorTxRunConfig,
        specific_state_tests: Option<F>,
    ) -> Result<(), InspectorTestUtilsError>
    where
        F: Fn(Box<dyn SpecificMev>),
    {
        let copied = config.clone();
        let err = || InspectorTestUtilsError::InspectorConfig(copied.clone());

        let inspector = config.expected_mev_type.ok_or_else(err)?;
        let tx_hashes = config.mev_tx_hashes.ok_or_else(err)?;
        let profit_usd = config.expected_profit_usd.ok_or_else(err)?;
        let gas_used_usd = config.expected_gas_usd.ok_or_else(err)?;

        let mut trees = self.classifier_inspector.build_tree_txes(tx_hashes).await?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|t| t.header.number).collect(),
            ))
        }
        let tree = trees.remove(0);
        let block = tree.header.number;

        let metadata = if let Some(meta) = config.metadata_override {
            meta
        } else {
            self.classifier_inspector.get_metadata(block).await?
        };

        let inspector = match inspector {
            MevType::Jit => {
                Box::new(JitInspector::new(self.quote_address, self.classifier_inspector.libmdbx))
                    as Box<dyn Inspector>
            }
            MevType::CexDex => Box::new(CexDexInspector::new(
                self.quote_address,
                self.classifier_inspector.libmdbx,
            )) as Box<dyn Inspector>,
            MevType::Backrun => Box::new(AtomicBackrunInspector::new(
                self.quote_address,
                self.classifier_inspector.libmdbx,
            )),
            MevType::Sandwich => Box::new(SandwichInspector::new(
                self.quote_address,
                self.classifier_inspector.libmdbx,
            )),
            missing => return Err(InspectorTestUtilsError::MissingInspector(missing)),
        };

        let mut results = inspector.process_tree(tree.into(), metadata.into()).await;
        assert_eq!(results.len(), 1, "more than 1 result for a single mev opp test");

        let (classified_mev, specific) = results.remove(0);

        if let Some(specific_state_tests) = specific_state_tests {
            specific_state_tests(specific);
        }

        // check gas
        assert!(
            (classified_mev.finalized_bribe_usd - gas_used_usd).abs() < self.max_result_difference,
            "Finalized Bribe != Expected Bribe"
        );
        // check profit
        assert!(
            (classified_mev.finalized_profit_usd - profit_usd).abs() < self.max_result_difference,
            "Finalized Profit != Expected Profit"
        );

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum InspectorTestUtilsError {
    #[error(transparent)]
    Classification(#[from] ClassifierTestUtilsError),
    #[error(transparent)]
    Tracing(#[from] TraceLoaderError),
    #[error("invalid inspector tx run config: {0:?}")]
    InspectorConfig(InspectorTxRunConfig),
    #[error("invalid composer run config: {0:?}")]
    ComposerConfig(ComposerRunConfig),
    #[error("no inspector for type: {0}")]
    MissingInspector(MevType),
    #[error("more than one block found in inspector config. blocks: {0:?}")]
    MultipleBlockError(Vec<u64>),
}
