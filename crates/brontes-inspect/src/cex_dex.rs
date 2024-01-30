use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::cex::CexExchange,
    mev::{
        Bundle, BundleData, CexDex, MevType, StatArbDetails, StatArbPnl, TokenProfit, TokenProfits,
    },
    normalized_actions::{Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    Root, ToFloatNearest,
};
use malachite::{
    num::basic::traits::{Two, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;
use tracing::debug;

use crate::{shared_utils::SharedInspectorUtils, BundleHeader, Inspector, MetadataCombined};

pub struct CexDexInspector<'db, DB: LibmdbxReader> {
    inner:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB, cex_exchanges: &Vec<CexExchange>) -> Self {
        Self {
            inner:         SharedInspectorUtils::new(quote, db),
            cex_exchanges: cex_exchanges.clone(),
        }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for CexDexInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<MetadataCombined>,
    ) -> Vec<Bundle> {
        // Get all normalized swaps by tx
        let intersting_state = tree.collect_all(|node| {
            (node.data.is_swap(), node.subactions.iter().any(|action| action.is_swap()))
        });

        intersting_state
            .into_par_iter()
            .filter(|(_, swaps)| !swaps.is_empty())
            .filter_map(|(tx, swaps)| {
                let root = tree.get_root(tx)?;

                self.process_swaps(root, meta_data.clone(), swaps)
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> CexDexInspector<'_, DB> {
    fn process_swaps(
        &self,
        root: &Root<Actions>,
        metadata: Arc<MetadataCombined>,
        swaps: Vec<Actions>,
    ) -> Option<Bundle> {
        let tx_index = root.get_block_position();
        let gas_details = root.gas_details;
        let mev_contract = root.head.data.get_to_address();
        let eoa = root.head.address;

        let swaps_with_profit_by_exchange: Vec<(
            NormalizedSwap,
            Vec<(CexExchange, Rational, StatArbPnl)>,
        )> = swaps
            .into_iter()
            .filter_map(|action| {
                let swap = action.force_swap();

                let cex_dex_opportunity =
                    self.detect_cex_dex_opportunity(&swap, metadata.as_ref())?;

                Some((swap, cex_dex_opportunity))
            })
            .collect();

        let possible_cex_dex =
            self.gas_accounting(swaps_with_profit_by_exchange, &gas_details, &metadata.eth_prices);

        let cex_dex = self.filter_possible_cex_dex(&possible_cex_dex, root)?;

        let header = self.inner.build_bundle_header(
            root,
            metadata,
            &vec![root.gas_details],
            (&vec![possible_cex_dex
                .swaps
                .iter()
                .map(|s| Actions::Swap(s.clone()))
                .collect()]),
            MevType::CexDex,
            possible_cex_dex.pnl.taker_profit,
        );

        Some(Bundle { header, data: cex_dex })
    }

    pub fn detect_cex_dex_opportunity(
        &self,
        swap: &NormalizedSwap,
        metadata: &MetadataCombined,
    ) -> Option<Vec<(CexExchange, Rational, StatArbPnl)>> {
        let cex_prices = self.cex_quotes_for_swap(swap, metadata)?;

        let opportunities = cex_prices
            .into_iter()
            .map(|(exchange, price, is_direct_pair)| {
                self.profit_classifier(swap, (exchange, price, is_direct_pair))
            })
            .collect();

        Some(opportunities)
    }

    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: (CexExchange, Rational, bool),
    ) -> (CexExchange, Rational, StatArbPnl) {
        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let delta_price = &exchange_cex_price.1 - swap.swap_rate();

        // Accounts for Cex Maker & Taker fees
        if exchange_cex_price.2 {
            // Direct pair
            (
                exchange_cex_price.0,
                exchange_cex_price.1,
                StatArbPnl {
                    maker_profit: &delta_price * &swap.amount_out
                        - &swap.amount_out * exchange_cex_price.0.fees().0,
                    taker_profit: delta_price * &swap.amount_out
                        - &swap.amount_out * exchange_cex_price.0.fees().1,
                },
            )
        } else {
            // Indirect pair pays twice the fee
            (
                exchange_cex_price.0,
                exchange_cex_price.1,
                StatArbPnl {
                    maker_profit: &delta_price * &swap.amount_out
                        - &swap.amount_out * exchange_cex_price.0.fees().0 * Rational::TWO,
                    taker_profit: delta_price * &swap.amount_out
                        - &swap.amount_out * exchange_cex_price.0.fees().1 * Rational::TWO,
                },
            )
        }
    }

    /// Gets the Cex quote for a Dex swap by exchange
    /// Retrieves CEX quotes for a DEX swap, grouped by exchange.
    /// If the quote is not found for a given exchange, it will try to find a
    /// quote via an intermediary token. Direct quotes are marked as
    /// `true`, intermediary quotes are marked as `false`. Which allows us to
    /// account for the additional fees.
    fn cex_quotes_for_swap(
        &self,
        swap: &NormalizedSwap,
        metadata: &MetadataCombined,
    ) -> Option<Vec<(CexExchange, Rational, bool)>> {
        let pair = Pair(swap.token_in.address, swap.token_out.address).ordered();
        let quotes = self
            .cex_exchanges
            .iter()
            .filter_map(|&exchange| {
                metadata
                    .db
                    .cex_quotes
                    .get_quote(&pair, &exchange)
                    .map(|cex_quote| (exchange, cex_quote.price.0, true))
                    .or_else(|| {
                        metadata
                            .db
                            .cex_quotes
                            .get_quote_via_intermediary(&pair, &exchange)
                            .map(|cex_quote| (exchange, cex_quote.price.0, false))
                    })
                    .or_else(|| {
                        debug!(
                            "No CEX quote found for pair: {}, {} at exchange: {:?}",
                            swap.token_in, swap.token_out, exchange
                        );
                        None
                    })
            })
            .collect::<Vec<_>>();

        if quotes.is_empty() {
            None
        } else {
            Some(quotes)
        }
    }

    fn gas_accounting(
        &self,
        swaps_with_profit_by_exchange: Vec<(
            NormalizedSwap,
            Vec<(CexExchange, Rational, StatArbPnl)>,
        )>,
        gas_details: &GasDetails,
        eth_price: &Rational,
    ) -> PossibleCexDex {
        // Get the maximally profitable sequence of Cex arbs by picking the most
        // profitable exchange to execute the arb for each swap (taker profits)
        let (swaps, arb_details, total_arb_pre_gas) = swaps_with_profit_by_exchange
            .into_iter()
            .filter_map(|(swap, net_profits_by_exchange)| {
                net_profits_by_exchange
                    .into_iter()
                    .max_by(|(_, _, profit1), (_, _, profit2)| {
                        profit1.taker_profit.cmp(&profit2.taker_profit)
                    }) // Compare based on maker_profit
                    .map(|(exchange, cex_price, profit)| (swap, exchange, cex_price, profit))
            })
            .fold(
                (Vec::new(), Vec::new(), StatArbPnl::default()),
                |(mut swaps, mut arb_details, mut total_profit),
                 (swap, exchange, cex_price, profit)| {
                    swaps.push(swap.clone());
                    arb_details.push(StatArbDetails {
                        cex_exchange: exchange,
                        cex_price,
                        dex_exchange: swap.protocol,
                        dex_price: swap.swap_rate(),
                        pnl_pre_gas: profit.clone(),
                    });
                    total_profit.maker_profit += profit.maker_profit;
                    total_profit.taker_profit += profit.taker_profit;
                    (swaps, arb_details, total_profit)
                },
            );

        let gas_cost = Rational::from_unsigneds(gas_details.gas_paid(), 10u128.pow(18)) * eth_price;
        let pnl = StatArbPnl {
            maker_profit: total_arb_pre_gas.maker_profit - &gas_cost,
            taker_profit: total_arb_pre_gas.taker_profit - gas_cost,
        };

        PossibleCexDex { swaps, arb_details, gas_details: gas_details.clone(), pnl }
    }

    fn filter_possible_cex_dex(
        &self,
        possible_cex_dex: &PossibleCexDex,
        root: &Root<Actions>,
    ) -> Option<BundleData> {
        // Check for positive pnl (either maker or taker profit)
        let has_positive_pnl = possible_cex_dex.pnl.maker_profit > Rational::ZERO
            || possible_cex_dex.pnl.taker_profit > Rational::ZERO;

        // A cex-dex bot will never be verified, so if the top level call is classified
        // this is false positive
        let is_unclassified_action = root.head.data.is_unclassified();

        // Check if it is a known cex_dex contract / contract call
        let is_known_cex_dex_contract =
            matches!(&root.head.data, Actions::Unclassified(data) if data.is_cex_dex_call());

        if (has_positive_pnl || possible_cex_dex.gas_details.coinbase_transfer.is_some())
            && is_unclassified_action
            || is_known_cex_dex_contract
        {
            Some(possible_cex_dex.build_cex_dex_type(root))
        } else {
            None
        }
    }
}

pub struct PossibleCexDex {
    pub swaps:       Vec<NormalizedSwap>,
    pub arb_details: Vec<StatArbDetails>,
    pub gas_details: GasDetails,
    pub pnl:         StatArbPnl,
}

impl PossibleCexDex {
    pub fn build_cex_dex_type(&self, root: &Root<Actions>) -> BundleData {
        BundleData::CexDex(CexDex {
            tx_hash:          root.tx_hash,
            gas_details:      self.gas_details.clone(),
            swaps:            self.swaps.clone(),
            stat_arb_details: self.arb_details.clone(),
            pnl:              self.pnl.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        str::FromStr,
    };

    use alloy_primitives::{hex, B256, U256};
    use brontes_types::db::cex::{CexPriceMap, CexQuote};
    use malachite::num::arithmetic::traits::Reciprocal;
    use serial_test::serial;

    use super::*;
    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[tokio::test]
    #[serial]
    async fn test_cex_dex() {
        // sold eth to buy usdc on chain
        let tx_hash =
            B256::from_str("0x21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295")
                .unwrap();

        // reciprocal because we store the prices as usdc / eth due to pair ordering
        let eth_price = Rational::try_from_float_simplest(1665.81)
            .unwrap()
            .reciprocal();
        let eth_cex = Rational::try_from_float_simplest(1645.81)
            .unwrap()
            .reciprocal();

        let eth_usdc = Pair(
            hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").into(),
            hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").into(),
        );
        let mut cex_map = HashMap::new();
        cex_map.insert(
            eth_usdc.ordered(),
            vec![CexQuote {
                price: (eth_cex.clone(), eth_cex),
                token0: Address::new(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
                ..Default::default()
            }],
        );

        let cex_quotes = CexPriceMap(cex_map);

        let metadata = MetadataCombined {
            dex_quotes: brontes_types::db::dex::DexQuotes(vec![Some({
                let mut map = HashMap::new();
                map.insert(eth_usdc, eth_price.clone());
                map
            })]),
            db:         brontes_types::db::metadata::MetadataNoDex {
                block_num: 18264694,
                block_hash: U256::from_be_bytes(hex!(
                    "57968198764731c3fcdb0caff812559ce5035aabade9e6bcb2d7fcee29616729"
                )),
                block_timestamp: 0,
                relay_timestamp: None,
                p2p_timestamp: None,
                proposer_fee_recipient: Some(
                    hex!("95222290DD7278Aa3Ddd389Cc1E1d165CC4BAfe5").into(),
                ),
                proposer_mev_reward: None,
                cex_quotes,
                eth_prices: eth_price.reciprocal(),
                private_flow: HashSet::new(),
            },
        };

        let test_utils = InspectorTestUtils::new(USDC_ADDRESS, 2.0);

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_metadata_override(metadata)
            .with_mev_tx_hashes(vec![tx_hash])
            .with_gas_paid_usd(79836.4183)
            .with_expected_profit_usd(21270.966);

        test_utils.run_inspector(config, None).await.unwrap();
    }
}
