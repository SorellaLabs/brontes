use alloy_primitives::{hex, Address, FixedBytes, TxHash};
use brontes_classifier::test_utils::{ClassifierTestUtils, ClassifierTestUtilsError};
use brontes_core::TraceLoaderError;
use brontes_types::{
    classified_mev::{MevType, SpecificMev},
    db::{dex::DexQuotes, metadata::MetadataCombined},
    normalized_actions::Actions,
    tree::BlockTree,
};
use thiserror::Error;

use crate::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector,
    composer::compose_mev_results, jit::JitInspector, liquidations::LiquidationInspector,
    sandwich::SandwichInspector, Inspector,
};

pub const USDC_ADDRESS: Address =
    Address(FixedBytes::<20>(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")));

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

    async fn get_tree_txes(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<BlockTree<Actions>, InspectorTestUtilsError> {
        let mut trees = self.classifier_inspector.build_tree_txes(tx_hashes).await?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|t| t.header.number).collect(),
            ))
        }
        Ok(trees.remove(0))
    }

    async fn get_tree_txes_with_pricing(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<(BlockTree<Actions>, DexQuotes), InspectorTestUtilsError> {
        let mut trees = self
            .classifier_inspector
            .build_tree_txes_with_pricing(tx_hashes, self.quote_address)
            .await?;

        if trees.len() != 1 {
            return Err(InspectorTestUtilsError::MultipleBlockError(
                trees.into_iter().map(|(t, _)| t.header.number).collect(),
            ))
        }
        Ok(trees.remove(0))
    }

    async fn get_tree_block(
        &self,
        block: u64,
    ) -> Result<BlockTree<Actions>, InspectorTestUtilsError> {
        self.classifier_inspector
            .build_tree_block(block)
            .await
            .map_err(Into::into)
    }

    async fn get_tree_block_with_pricing(
        &self,
        block: u64,
    ) -> Result<(BlockTree<Actions>, DexQuotes), InspectorTestUtilsError> {
        self.classifier_inspector
            .build_tree_block_with_pricing(block, self.quote_address)
            .await
            .map_err(Into::into)
    }

    fn get_inspector(
        &self,
        mev_type: MevType,
    ) -> Result<Box<dyn Inspector>, InspectorTestUtilsError> {
        let inspector = match mev_type {
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
            MevType::Liquidation => Box::new(LiquidationInspector::new(
                self.quote_address,
                self.classifier_inspector.libmdbx,
            )),
            missing => return Err(InspectorTestUtilsError::MissingInspector(missing)),
        };
        Ok(inspector)
    }

    fn get_inspectors(
        &self,
        mev_types: Vec<MevType>,
    ) -> Result<Vec<Box<dyn Inspector>>, InspectorTestUtilsError> {
        let mut res = Vec::new();
        for mev_type in mev_types {
            let inspector = match mev_type {
                MevType::Jit => Box::new(JitInspector::new(
                    self.quote_address,
                    self.classifier_inspector.libmdbx,
                )) as Box<dyn Inspector>,
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
                MevType::Liquidation => Box::new(LiquidationInspector::new(
                    self.quote_address,
                    self.classifier_inspector.libmdbx,
                )),
                missing => return Err(InspectorTestUtilsError::MissingInspector(missing)),
            };
            res.push(inspector);
        }
        Ok(res)
    }

    pub async fn run_inspector(
        &self,
        config: InspectorTxRunConfig,
        specific_state_tests: Option<Box<dyn Fn(SpecificMev)>>,
    ) -> Result<(), InspectorTestUtilsError> {
        let copied = config.clone();
        let err = || InspectorTestUtilsError::InspectorConfig(copied.clone());

        let profit_usd = config.expected_profit_usd.ok_or_else(err)?;
        let gas_used_usd = config.expected_gas_usd.ok_or_else(err)?;

        let mut quotes = None;
        let tree = if let Some(tx_hashes) = config.mev_tx_hashes {
            if config.calculate_dex_prices {
                let (tree, prices) = self.get_tree_txes_with_pricing(tx_hashes).await?;
                quotes = Some(prices);
                tree
            } else {
                self.get_tree_txes(tx_hashes).await?
            }
        } else if let Some(block) = config.mev_block {
            if config.calculate_dex_prices {
                let (tree, prices) = self.get_tree_block_with_pricing(block).await?;
                quotes = Some(prices);
                tree
            } else {
                self.get_tree_block(block).await?
            }
        } else {
            return Err(err())
        };

        let block = tree.header.number;

        let mut metadata = if let Some(meta) = config.metadata_override {
            meta
        } else {
            self.classifier_inspector.get_metadata(block).await?
        };

        if let Some(quotes) = quotes {
            metadata.dex_quotes = quotes;
        }

        if metadata.dex_quotes.0.is_empty() {
            assert!(false, "no dex quotes found in metadata. test suite will fail");
        }

        let inspector = self.get_inspector(config.expected_mev_type)?;

        let mut results = inspector.process_tree(tree.into(), metadata.into()).await;
        assert_eq!(results.len(), 1, "got a non zero amount of detected mev {:#?}", results);

        let (classified_mev, specific) = results.remove(0);

        if let Some(specific_state_tests) = specific_state_tests {
            specific_state_tests(specific);
        }

        // check gas
        assert!(
            (classified_mev.finalized_bribe_usd - gas_used_usd).abs() < self.max_result_difference,
            "Finalized Bribe != Expected Bribe, {} != {}",
            classified_mev.finalized_bribe_usd,
            gas_used_usd
        );
        // check profit
        assert!(
            (classified_mev.finalized_profit_usd - profit_usd).abs() < self.max_result_difference,
            "Finalized Profit != Expected Profit, {} != {}",
            classified_mev.finalized_profit_usd,
            profit_usd
        );

        Ok(())
    }

    pub async fn run_composer(
        &self,
        config: ComposerRunConfig,
        specific_state_tests: Option<Box<dyn Fn(SpecificMev)>>,
    ) -> Result<(), InspectorTestUtilsError> {
        let copied = config.clone();
        let err = || InspectorTestUtilsError::ComposerConfig(copied.clone());

        let profit_usd = config.expected_profit_usd.ok_or_else(err)?;
        let gas_used_usd = config.expected_gas_usd.ok_or_else(err)?;

        let mut quotes = None;
        let tree = if let Some(tx_hashes) = config.mev_tx_hashes {
            if config.calculate_dex_prices {
                let (tree, prices) = self.get_tree_txes_with_pricing(tx_hashes).await?;
                quotes = Some(prices);
                tree
            } else {
                self.get_tree_txes(tx_hashes).await?
            }
        } else if let Some(block) = config.mev_block {
            if config.calculate_dex_prices {
                let (tree, prices) = self.get_tree_block_with_pricing(block).await?;
                quotes = Some(prices);
                tree
            } else {
                self.get_tree_block(block).await?
            }
        } else {
            return Err(err())
        };

        let block = tree.header.number;

        let mut metadata = if let Some(meta) = config.metadata_override {
            meta
        } else {
            self.classifier_inspector.get_metadata(block).await?
        };

        if let Some(quotes) = quotes {
            metadata.dex_quotes = quotes;
        }

        if metadata.dex_quotes.0.is_empty() {
            assert!(false, "no dex quotes found in metadata. test suite will fail");
        }

        let inspector = self
            .get_inspectors(config.inspectors)?
            .into_iter()
            .map(|i| &*Box::leak(Box::new(i)))
            .collect::<Vec<&'static Box<dyn Inspector>>>();

        let (_, results) =
            compose_mev_results(inspector.as_slice(), tree.into(), metadata.into()).await;

        let mut results = results
            .into_iter()
            .filter(|mev| {
                config
                    .prune_opportunities
                    .as_ref()
                    .map(|opp| !opp.contains(&mev.0.tx_hash))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 1, "got a non zero amount of detected mev");

        let (classified_mev, specific) = results.remove(0);
        assert!(classified_mev.mev_type == config.expected_mev_type, "got wrong composed type");

        if let Some(specific_state_tests) = specific_state_tests {
            specific_state_tests(specific);
        }

        // check gas
        assert!(
            (classified_mev.finalized_bribe_usd - gas_used_usd).abs() < self.max_result_difference,
            "Finalized Bribe != Expected Bribe, {} != {}",
            classified_mev.finalized_bribe_usd,
            gas_used_usd
        );
        // check profit
        assert!(
            (classified_mev.finalized_profit_usd - profit_usd).abs() < self.max_result_difference,
            "Finalized Profit != Expected Profit, {} != {}",
            classified_mev.finalized_profit_usd,
            profit_usd
        );

        Ok(())
    }
}

/// This run config is used for a single opportunity test.
/// it supports multiple hashes for sandwiches
#[derive(Debug, Clone)]
pub struct InspectorTxRunConfig {
    pub metadata_override:    Option<MetadataCombined>,
    pub mev_tx_hashes:        Option<Vec<TxHash>>,
    pub mev_block:            Option<u64>,
    pub expected_profit_usd:  Option<f64>,
    pub expected_gas_usd:     Option<f64>,
    pub expected_mev_type:    MevType,
    pub calculate_dex_prices: bool,
}

impl InspectorTxRunConfig {
    pub fn new(mev: MevType) -> Self {
        Self {
            expected_mev_type:    mev,
            mev_block:            None,
            mev_tx_hashes:        None,
            expected_profit_usd:  None,
            expected_gas_usd:     None,
            metadata_override:    None,
            calculate_dex_prices: false,
        }
    }

    pub fn with_dex_prices(mut self) -> Self {
        self.calculate_dex_prices = true;
        self
    }

    pub fn with_block(mut self, block: u64) -> Self {
        self.mev_block = Some(block);
        self
    }

    pub fn with_metadata_override(mut self, metadata: MetadataCombined) -> Self {
        self.metadata_override = Some(metadata);
        self
    }

    pub fn with_mev_tx_hashes(mut self, txes: Vec<TxHash>) -> Self {
        self.mev_tx_hashes = Some(txes);
        self
    }

    pub fn with_expected_profit_usd(mut self, profit: f64) -> Self {
        self.expected_profit_usd = Some(profit);
        self
    }

    pub fn with_expected_gas_used(mut self, gas: f64) -> Self {
        self.expected_gas_usd = Some(gas);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ComposerRunConfig {
    pub inspectors:           Vec<MevType>,
    pub expected_mev_type:    MevType,
    pub metadata_override:    Option<MetadataCombined>,
    pub mev_tx_hashes:        Option<Vec<TxHash>>,
    pub mev_block:            Option<u64>,
    pub expected_profit_usd:  Option<f64>,
    pub expected_gas_usd:     Option<f64>,
    pub prune_opportunities:  Option<Vec<TxHash>>,
    pub calculate_dex_prices: bool,
}

impl ComposerRunConfig {
    pub fn new(inspectors: Vec<MevType>, expected_mev_type: MevType) -> Self {
        Self {
            inspectors,
            metadata_override: None,
            mev_tx_hashes: None,
            expected_mev_type,
            mev_block: None,
            expected_profit_usd: None,
            expected_gas_usd: None,
            prune_opportunities: None,
            calculate_dex_prices: false,
        }
    }

    pub fn with_metadata_override(mut self, metadata: MetadataCombined) -> Self {
        self.metadata_override = Some(metadata);
        self
    }

    pub fn with_mev_tx_hashes(mut self, txes: Vec<TxHash>) -> Self {
        self.mev_tx_hashes = Some(txes);
        self
    }

    pub fn with_block(mut self, block: u64) -> Self {
        self.mev_block = Some(block);
        self
    }

    pub fn with_expected_profit_usd(mut self, profit: f64) -> Self {
        self.expected_profit_usd = Some(profit);
        self
    }

    pub fn with_expected_gas_used(mut self, gas: f64) -> Self {
        self.expected_gas_usd = Some(gas);
        self
    }

    pub fn with_prune_opportunities(mut self, prune_txes: Vec<TxHash>) -> Self {
        self.prune_opportunities = Some(prune_txes);
        self
    }

    pub fn with_dex_prices(mut self) -> Self {
        self.calculate_dex_prices = true;
        self
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
