use std::sync::Arc;

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    classified_mev::{Bundle, BundleData, CexDex, MevType, PriceKind, TokenProfit, TokenProfits},
    extra_processing::Pair,
    normalized_actions::{Actions, NormalizedSwap},
    tree::{BlockTree, GasDetails},
    ToFloatNearest, ToScaledRational,
};
use malachite::{num::basic::traits::Zero, Rational};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    prelude::IntoParallelRefIterator,
};
use reth_primitives::{Address, B256};
use tracing::{debug, error, trace};

use crate::{shared_utils::SharedInspectorUtils, BundleHeader, Inspector, MetadataCombined};

pub struct CexDexInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> CexDexInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}
//TODO: Support for multiple CEXs
//TODO: Filtering by coinbase.transfer() to builder or directly to the proposer
//TODO: Start adding filtering by function sig in the tree. Like executeFFsYo
//TODO: If single swap with coinbase.transfer then flag token as missing cex
// price (addr + symbol + name) in a db table so we can fill in what is missing

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for CexDexInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<MetadataCombined>,
    ) -> Vec<Bundle> {
        // Get all normalized swaps
        let intersting_state = tree.collect_all(|node| {
            (node.data.is_swap(), node.subactions.iter().any(|action| action.is_swap()))
        });

        intersting_state
            .into_par_iter()
            .filter(|(_, swaps)| !swaps.is_empty())
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?;

                let root = tree.get_root(tx)?;
                let eoa = root.head.address;
                let mev_contract = root.head.data.get_to_address();
                let idx = root.get_block_position();
                self.process_swaps(
                    tx,
                    idx,
                    mev_contract,
                    eoa,
                    meta_data.clone(),
                    gas_details,
                    swaps,
                )
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> CexDexInspector<'_, DB> {
    fn process_swaps(
        &self,
        hash: B256,
        idx: usize,
        mev_contract: Address,
        eoa: Address,
        metadata: Arc<MetadataCombined>,
        gas_details: &GasDetails,
        swaps: Vec<Actions>,
    ) -> Option<Bundle> {
        let swap_sequences: Vec<(&Actions, _)> = swaps
            .iter()
            .filter_map(|action| {
                if let Actions::Swap(ref normalized_swap) = action {
                    Some((action, self.get_cex_dex(idx, normalized_swap, metadata.as_ref())))
                } else {
                    None
                }
            })
            .collect();

        let profit = self.arb_gas_accounting(swap_sequences, gas_details, &metadata.eth_prices);
        let gas_finalized = metadata.get_gas_price_usd(gas_details.gas_paid());
        let deltas = self.inner.calculate_token_deltas(&vec![swaps.clone()]);

        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx, true, &deltas, metadata.clone(), true)?;

        let mev_profit_collector = self.inner.profit_collectors(&addr_usd_deltas);

        let token_profits = TokenProfits {
            profits: mev_profit_collector
                .iter()
                .filter_map(|address| deltas.get(address).map(|d| (address, d)))
                .flat_map(|(address, delta)| {
                    delta.iter().map(|(token, amount)| {
                        let usd_value = metadata
                            .cex_quotes
                            .get_quote(&Pair(*token, self.inner.quote))
                            .unwrap_or_default()
                            .price
                            .1
                            .to_float()
                            * amount.clone().to_float();
                        TokenProfit {
                            profit_collector: *address,
                            token: *token,
                            amount: amount.clone().to_float(),
                            usd_value,
                        }
                    })
                })
                .collect(),
        };

        let header = BundleHeader {
            tx_index: idx as u64,
            mev_profit_collector,
            tx_hash: hash,
            mev_contract,
            eoa,
            block_number: metadata.block_num,
            mev_type: MevType::CexDex,
            profit_usd: profit?.to_float(),
            token_profits,
            bribe_usd: gas_finalized.to_float(),
        };

        let prices = swaps
            .par_iter()
            .filter_map(|swap| self.rational_prices(idx, swap, &metadata))
            .map(|(dex_price, cex1)| (dex_price.to_float(), cex1.to_float()))
            .collect::<Vec<_>>();

        let flat_swaps = swaps
            .into_iter()
            .filter(|act| act.is_swap())
            .map(|swap| swap.force_swap())
            .collect::<Vec<_>>();

        let cex_dex = CexDex {
            tx_hash:        hash,
            gas_details:    gas_details.clone(),
            swaps:          flat_swaps.clone(),
            prices_kind:    prices
                .iter()
                .flat_map(|_| vec![PriceKind::Dex, PriceKind::Cex])
                .collect(),
            prices_address: flat_swaps
                .iter()
                .flat_map(|s| vec![s.token_in].repeat(2))
                .collect(),
            prices_price:   prices
                .iter()
                .flat_map(|(dex, cex)| vec![*dex, *cex])
                .collect(),
        };

        Some(Bundle { header, data: BundleData::CexDex(cex_dex) })
    }

    fn arb_gas_accounting(
        &self,
        swap_sequences: Vec<(&Actions, Option<Rational>)>,
        gas_details: &GasDetails,
        eth_price: &Rational,
    ) -> Option<Rational> {
        let zero = Rational::ZERO;
        let total_arb = swap_sequences
            .iter()
            .fold(Rational::ZERO, |acc, (_, v)| acc + v.as_ref().unwrap_or(&zero));

        let gas_cost = Rational::from_unsigneds(gas_details.gas_paid(), 10u128.pow(18)) * eth_price;

        if total_arb > gas_cost || gas_details.coinbase_transfer.is_some() {
            Some(total_arb - gas_cost)
        } else {
            None
        }
    }

    pub fn get_cex_dex(
        &self,
        tx_idx: usize,
        swap: &NormalizedSwap,
        metadata: &MetadataCombined,
    ) -> Option<Rational> {
        self.rational_prices(tx_idx, &Actions::Swap(swap.clone()), metadata)
            .and_then(|(dex_price, best_ask)| self.profit_classifier(swap, &dex_price, &best_ask))
    }

    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        dex_price: &Rational,
        cex_price: &Rational,
    ) -> Option<Rational> {
        // Calculate the price differences between DEX and CEX
        let delta_price = dex_price - cex_price;

        // Calculate the potential profit
        let Ok(Some(decimals_in)) = self.inner.db.try_get_token_decimals(swap.token_in) else {
            error!(missing_token=?swap.token_in, "missing token in token to decimal map");
            return None
        };

        Some(delta_price * swap.amount_in.to_scaled_rational(decimals_in))
    }

    pub fn rational_prices(
        &self,
        tx_idx: usize,
        swap: &Actions,
        metadata: &MetadataCombined,
    ) -> Option<(Rational, Rational)> {
        let Actions::Swap(swap) = swap else { return None };

        let pair_in = Pair(swap.token_in, self.inner.quote);
        let pair_out = Pair(swap.token_out, self.inner.quote);

        let in_usd = metadata
            .dex_quotes
            .price_at_or_before(pair_in, tx_idx)?
            .post_state;
        let out_usd = metadata
            .dex_quotes
            .price_at_or_before(pair_out, tx_idx)?
            .post_state;

        let dex_usd_price = out_usd / in_usd;

        let cex_best_ask = match (
            metadata.cex_quotes.get_quote(&pair_in),
            metadata.cex_quotes.get_quote(&pair_out),
        ) {
            (Some(token_in_price), Some(token_out_price)) => {
                trace!(
                    "CEX quote found for pair: {}, {} at block: {}",
                    swap.token_in,
                    swap.token_out,
                    metadata.block_num
                );
                token_out_price.best_ask() / token_in_price.best_ask()
            }
            (..) => {
                debug!(
                    "No CEX quote found for pair: {}, {} at block: {}",
                    swap.token_in, swap.token_out, metadata.block_num
                );
                return None
            }
        };

        Some((dex_usd_price, cex_best_ask))
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
                mempool_flow: HashSet::new(),
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
