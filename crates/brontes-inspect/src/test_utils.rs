use alloy_primitives::{Address, TxHash};
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_database::Metadata;
use brontes_types::classified_mev::MevType;
use malachite::Rational;
use thiserror::Error;

use crate::{cex_dex::CexDexInspector, jit::JitInspector, Inspector, atomic_backrun::AtomicBackrunInspector, long_tail::LongTailInspector};

#[derive(Debug, Clone)]
pub struct InspectorTxRunConfig {
    pub metadata_override: Option<Metadata>,
    pub block:             Option<u64>,
    pub mev_tx_hashes:     Option<Vec<TxHash>>,
    pub expected_profit:   Option<Rational>,
    pub expected_mev_type: Option<MevType>,
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
    classifier_inspector:         ClassifierTestUtils,
    quote_address:                Address,
    max_result_profit_difference: Rational,
}

impl InspectorTestUtils {
    pub fn new(quote_address: Address, max_result_profit_difference: Rational) -> Self {
        let classifier_inspector = ClassifierTestUtils::new();
        Self { classifier_inspector, quote_address, max_result_profit_difference }
    }

    pub async fn run_inspector(
        &self,
        config: InspectorTxRunConfig,
    ) -> Result<(), InspectorTestUtilsError> {
        let err = || InspectorTestUtilsError::InspectorConfig(config.clone());

        let inspector = config.expected_mev_type.ok_or_else(err)?;

        let inspector = match inspector {
            MevType::Jit => {
                Box::new(JitInspector::new(self.quote_address, self.classifier_inspector.libmdbx))
                    as Box<dyn Inspector>
            }
            MevType::CexDex => Box::new(CexDexInspector::new(
                self.quote_address,
                self.classifier_inspector.libmdbx,
            )) as Box<dyn Inspector>,
            MevType::Backrun => Box::new(AtomicBackrunInspector::new(self.quote_address, self.classifier_inspector.libmdbx)),
            MevType::Sandwich => Box::new(LongTailInspector::new(self.quote_address, self.classifier_inspector.libmdbx)),
            missing => return Err(InspectorTestUtilsError::MissingInspector(missing)),
        };

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum InspectorTestUtilsError {
    #[error(transparent)]
    Classification(#[from] ClassifierTestUtilsError),
    #[error("invalid inspector tx run config: {0:?}")]
    InspectorConfig(InspectorTxRunConfig),
    #[error("invalid composer run config: {0:?}")]
    ComposerConfig(ComposerRunConfig),
    #[error("no inspector for type: {0}")]
    MissingInspector(MevType),
}
