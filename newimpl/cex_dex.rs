//! This module implements the `CexDexInspector`, a specialized inspector
//! designed to detect arbitrage opportunities between centralized
//! exchanges (CEXs) and decentralized exchanges (DEXs).
//!
//! ## Overview
//!
//! A Cex-Dex arbitrage occurs when a trader exploits the price difference
//! between a CEX and a DEX. The trader buys an undervalued asset on the DEX and
//! sells it on the CEX.
//!
//!
//! ## Methodology
//!
//! The `CexDexInspector` systematically identifies arbitrage opportunities
//! between CEXs and DEXs by analyzing transactions containing swap actions.
//!
//! ### Step 1: Collect Transactions
//! All transactions containing swap actions are collected from the block tree
//! using `collect_all`.
//!
//! ### Step 2: Detect Arbitrage Opportunities
//! For each transaction with swaps, the inspector:
//!   - Retrieves CEX quotes for the swapped tokens for each exchange with
//!     `cex_quotes_for_swap`.
//!   - Calculates PnL post Cex & Dex fee and identifies arbitrage legs with
//!     `detect_cex_dex_opportunity`, considering both direct and intermediary
//!     token quotes.
//!   - Assembles `PossibleCexDexLeg` instances, for each swap, containing the
//!     swap action and the potential arbitrage legs i.e the different
//!     arbitrages that can be done for each exchange.
//!
//! ### Step 3: Profit Calculation and Gas Accounting
//! The inspector filters for the most profitable arbitrage path per swap i.e
//! for a given swap it gets the exchange with the highest profit
//! through `filter_most_profitable_leg`. It then gets the total potential
//! profit, and accounts for gas costs with `gas_accounting` to calculate the
//! transactions final PnL.
//!
//! ### Step 4: Validation and Bundle Construction
//! Arbitrage opportunities are validated and false positives minimized in
//! `filter_possible_cex_dex`. Valid opportunities are bundled into
//! `BundleData::CexDex` instances.

use std::{collections::HashMap, sync::Arc};

use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{
        cex::{CexExchange, CexQuote, ExchangeData, Trade, TradeSide},
        dex::PriceAt,
    },
    mev::{Bundle, BundleData, CexDex, MevType, StatArbDetails, StatArbPnl},
    normalized_actions::{Actions, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails},
    ToFloatNearest, TxInfo,
};
use malachite::{
    num::basic::traits::{One, Two, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;
use tracing::debug;

use crate::{shared_utils::SharedInspectorUtils, Inspector, Metadata};

pub struct CexDexInspector<'db, DB: LibmdbxReader> {
    inner:         SharedInspectorUtils<'db, DB>,
    cex_exchanges: Vec<CexExchange>,
}

impl<'db, DB: LibmdbxReader> CexDexInspector<'db, DB> {
    /// Constructs a new `CexDexInspector`.
    ///
    /// # Arguments
    ///
    /// * `quote` - The address of the quote asset
    /// * `db` - Database reader to our local libmdbx database
    /// * `cex_exchanges` - List of centralized exchanges to consider for
    ///   arbitrage.
    pub fn new(quote: Address, db: &'db DB, cex_exchanges: &[CexExchange]) -> Self {
        Self {
            inner:         SharedInspectorUtils::new(quote, db),
            cex_exchanges: cex_exchanges.to_owned(),
        }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for CexDexInspector<'_, DB> {
    type Result = Vec<Bundle>;

    /// Processes the block tree to find CEX-DEX arbitrage
    /// opportunities. This is the entry point for the inspection process,
    /// identifying transactions that include swap actions.
    ///
    /// # Arguments
    /// * `tree` - A shared reference to the block tree.
    /// * `metadata` - Shared metadata struct containing:
    ///     - `cex_quotes` - CEX quotes
    ///     - `dex_quotes` - DEX quotes
    ///     - `private_flow` - Set of private transactions that were not seen in
    ///       the mempool
    ///     - `relay & p2p_timestamp` - When the block was first sent to a relay
    ///       & when it was first seen in the p2p network
    ///
    ///
    /// # Returns
    /// A vector of `Bundle` instances representing classified CEX-DEX arbitrage
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Self::Result {
        let swap_txes = tree.collect_all(|node| brontes_types::TreeSearchArgs {
            collect_current_node:  node.data.is_swap(),
            child_node_to_collect: node.subactions.iter().any(|action| action.is_swap()),
        });

        swap_txes
            .into_par_iter()
            .filter(|(_, swaps)| !swaps.is_empty())
            .filter_map(|(tx, swaps)| {
                let tx_info = tree.get_tx_info(tx, self.inner.db)?;

                // For each swap in the transaction, detect potential CEX-DEX
                let possible_cex_dex_by_exchange: Vec<PossibleCexDexLeg> = swaps
                    .into_iter()
                    .filter_map(|action| {
                        let swap = action.force_swap();

                        let possible_cex_dex =
                            self.detect_cex_dex_opportunity(&swap, metadata.as_ref())?;

                        Some(possible_cex_dex)
                    })
                    .collect();

                let possible_cex_dex = self.gas_accounting(
                    possible_cex_dex_by_exchange,
                    &tx_info.gas_details,
                    metadata.clone(),
                )?;

                let cex_dex =
                    self.filter_possible_cex_dex(&possible_cex_dex, &tx_info, metadata.clone())?;

                let header = self.inner.build_bundle_header(
                    &tx_info,
                    possible_cex_dex.pnl.taker_profit.clone().to_float(),
                    PriceAt::After,
                    &[possible_cex_dex.get_swaps()],
                    &[tx_info.gas_details],
                    metadata.clone(),
                    MevType::CexDex,
                );

                Some(Bundle { header, data: cex_dex })
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> CexDexInspector<'_, DB> {
    /// Detects potential CEX-DEX arbitrage opportunities for a given swap.
    ///
    /// # Arguments
    ///
    /// * `swap` - The swap action to analyze.
    /// * `metadata` - Combined metadata for additional context in analysis.
    ///
    /// # Returns
    ///
    /// An option containing a `PossibleCexDexLeg` if an opportunity is found,
    /// otherwise `None`.
    pub fn detect_cex_dex_opportunity(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<PossibleCexDexLeg> {
        let cex_prices = self.cex_quotes_for_swap(swap, metadata)?;

        let possible_legs: Vec<ExchangeLeg> = cex_prices
            .into_iter()
            .filter_map(|(exchange, price, is_direct_pair)| {
                self.profit_classifier(swap, (exchange, price, is_direct_pair), metadata)
            })
            .collect();

        Some(PossibleCexDexLeg { swap: swap.clone(), possible_legs })
    }

    /// For a given swap & CEX quote, calculates the potential profit from
    /// buying on DEX and selling on CEX. This function also accounts for CEX
    /// trading fees.
    fn profit_classifier(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: (CexExchange, Rational, bool),
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        // A positive delta indicates potential profit from buying on DEX
        // and selling on CEX.
        let delta_price = &exchange_cex_price.1 - swap.swap_rate();
        let fees = exchange_cex_price.0.fees();

        let token_price = metadata
            .cex_quotes
            .get_quote_direct_or_via_intermediary(
                &Pair(swap.token_in.address, self.inner.quote),
                &exchange_cex_price.0,
            )?
            .price
            .0;

        let (maker_profit, taker_profit) = if exchange_cex_price.2 {
            (
                (&delta_price * (&swap.amount_out - &swap.amount_out * fees.0)) * &token_price,
                (delta_price * (&swap.amount_out - &swap.amount_out * fees.1)) * &token_price,
            )
        } else {
            (
                // Indirect pair pays twice the fee
                (&delta_price * (&swap.amount_out - &swap.amount_out * fees.0 * Rational::TWO))
                    * &token_price,
                (delta_price * (&swap.amount_out - &swap.amount_out * fees.1 * Rational::TWO))
                    * &token_price,
            )
        };

        Some(ExchangeLeg {
            exchange:  exchange_cex_price.0,
            cex_price: exchange_cex_price.1,
            pnl:       StatArbPnl { maker_profit, taker_profit },
            is_direct: exchange_cex_price.2,
        })
    }

    fn get_primary_pair(
        &self,
        exchange_data: &ExchangeData,
        window_start: u64,
        window_end: u64,
    ) -> (Vec<Trade>, HashMap<u64, CexQuote>) {
        let mut pair_0_vol = Rational::ZERO;
        let mut pair_1_vol = Rational::ZERO;
        for trade in exchange_data.trades.0.iter() {
            if trade.timestamp < window_start {
                //it might be smart to cut out-of-window trades here but that should be
                // done on the db side at some point so I'm not going to implement it
                continue;
            }
            if trade.timestamp > window_end {
                break;
            }
            // we need to normalize volume to the intermediary asset so they can be compared
            pair_0_vol += &trade.amount * &trade.price;
            break;
        }
        for trade in exchange_data.trades.1.iter() {
            if trade.timestamp < window_start {
                //it might be smart to cut out-of-window trades here but that should be
                // done on the db side at some point so I'm not going to implement it
                continue;
            }
            if trade.timestamp > window_end {
                break;
            }
            // we need to normalize volume to the intermediary asset so they can be compared
            pair_1_vol += &trade.amount / &trade.price;
            break;
        }
        if pair_0_vol < pair_1_vol {
            (exchange_data.trades.1.clone(), exchange_data.quotes.0.clone())
        } else {
            (exchange_data.trades.0.clone(), exchange_data.quotes.1.clone())
        }
    }

    fn process_paths(
        &self,
        trades: Vec<Trade>,
        quotes: HashMap<u64, CexQuote>,
        fees: (Rational, Rational),
        swap_rate: Rational,
        window_start: u64,
        window_end: u64,
    ) -> (Rational, Rational, Rational, Rational, Rational) {
        // Note: We will sum all trades in the window and calculate the volume weighted
        // markout
        let mut total_taker_volume = Rational::ZERO;
        let mut total_taker_vwp = Rational::ZERO;
        let mut total_maker_volume = Rational::ZERO;
        let mut total_maker_vwp = Rational::ZERO;
        let indirect = !quotes.is_empty();
        for trade in trades.iter() {
            if trade.timestamp < window_start {
                continue;
            }
            if trade.timestamp > window_end {
                break;
            }
            match trade.side {
                // ### Process Trade
                // process trade based on whether it is a maker or taker
                // trade as fees differ
                //maker
                TradeSide::Buy => {
                    // Adjust the price for the maker fee
                    let mut adjusted_price = &trade.price * (Rational::ONE - &fees.0);
                    // If the pair is indirect we need to adjust the price for the secondary
                    // pair's best bid at the trade timestamp ### Notes:
                    // * We're assuming the arber is always immediately trading out of the
                    //   intermediary pair
                    if indirect {
                        adjusted_price = adjusted_price
                                // they have to pay taker fee again on second pair 
                                * (Rational::ONE - &fees.1)
                                * &quotes.get(&trade.timestamp).unwrap().price.1;
                    }
                    // ### Skip Unprofitable Trades
                    // Skip unprofitable trades if pair is direct
                    // ### Notes:
                    // * This assumes the arber is never forced to take bad trades which is
                    //   potentially false
                    // * A potential improvement here would be increasing the "tolerance" for bad
                    //   prices as it get's later in the window if total volume
                    // * still less than swap volume - as the assumption would be the arber hasn't
                    //   finished hedging and needs to clear inventory even at a loss
                    if adjusted_price < swap_rate {
                        continue;
                    }

                    total_maker_vwp += adjusted_price * &trade.amount;
                    total_maker_volume += &trade.amount
                }
                //taker
                _ => {
                    // Adjust the price for the taker fee
                    let mut adjusted_price = &trade.price * (Rational::ONE - &fees.1);
                    // If the pair is indirect we need to adjust the price for the secondary
                    // pair's best bid at the trade timestamp
                    if indirect {
                        adjusted_price = adjusted_price
                                // they have to pay taker fee again on second pair 
                                * (Rational::ONE - &fees.1)
                                * &quotes.get(&trade.timestamp).unwrap().price.1;
                    }
                    //skip unprofitable trades if pair is direct
                    if adjusted_price < swap_rate {
                        continue;
                    }
                    total_taker_vwp += adjusted_price * &trade.amount;
                    total_taker_volume += &trade.amount
                }
            };
        }
        let total_volume = &total_taker_volume + &total_maker_volume;
        let total_vwap = (&total_taker_vwp + &total_maker_vwp) / &total_volume;
        (total_taker_vwp, total_taker_volume, total_maker_vwp, total_maker_volume, total_vwap)
    }

    fn sum_paths(
        &self,
        paths: Vec<(Rational, Rational, Rational, Rational, Rational)>,
        needed_volume: Rational,
    ) -> (Rational, Rational, Rational, Rational) {
        let mut remaining_volume = needed_volume.clone();

        // Calculate total volume and volume weighter prices assuming that they were
        // trading on the best paths Note: This is optimistic as it assumes the
        // arber was ALL volume for that primary pair in that path - we could
        // apply winrates here to make it a more realistic measurment
        let mut total_taker_vwp = Rational::ZERO;
        let mut total_maker_vwp = Rational::ZERO;
        let mut total_taker_volume = Rational::ZERO;
        let mut total_maker_volume = Rational::ZERO;
        for (taker_volume, taker_vwp, maker_volume, maker_vwp, _) in paths.iter() {
            let total_volume = taker_volume + maker_volume;
            // If you don't need more volume axe the rest of the makrouts and break
            if total_volume >= remaining_volume {
                let path_pct_usage = &remaining_volume / total_volume;
                total_taker_vwp += taker_vwp * &path_pct_usage;
                total_maker_vwp += maker_vwp * &path_pct_usage;
                total_taker_volume += taker_volume * &path_pct_usage;
                total_maker_volume += maker_volume * &path_pct_usage;
                break;
            }
            total_taker_vwp += taker_vwp;
            total_maker_vwp += maker_vwp;
            total_taker_volume += taker_volume;
            total_maker_volume += maker_volume;

            remaining_volume -= total_volume;
        }
        (total_taker_vwp, total_taker_volume, total_maker_vwp, total_maker_volume)
    }

    fn profit_classifier_trade(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: (Vec<ExchangeData>, Rational, bool),
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        // ### Set Relevant Trade Window
        // Window is assumed to be 6 seconds ahead of the block and 6 seconds
        // after ### Notes:
        // * This is a very naive approach
        // * We could potentially improve it by examining whether markouts revert more
        //   on average before or after the block
        // * We could also improve it by setting it based on the pair- ETH:USDC is a
        //   competitive pair that arber's are comfortable holding inventory of
        // thus they are probably hedging before the block to secure positive
        // markouts for less competitive pairs they likely wait until
        // after the block to lower risk
        let window_start = metadata.block_timestamp - 6000;
        let window_end = metadata.block_timestamp + 6000;

        // all possible paths
        // (total_taker_volume,taker_vwp,total_maker_volume,maker_vwp,total_vwam)
        let mut possible_paths: Vec<(Rational, Rational, Rational, Rational, Rational)> =
            Vec::new();
        let fees: (Rational, Rational) = exchange_cex_price.0.first().unwrap().exchange.fees();

        for path in exchange_cex_price.0.iter() {
            // ### Determine Dominant Pair
            // if the path isn't direct we need to assign the dominant pair by figuring out
            // which pair has less volume ### Notes:
            // * The assumption made here is the lower volume pair is higher signal as more
            //   volume is arb related
            let (dominant_pair, secondary_quotes) = if !exchange_cex_price.2 {
                self.get_primary_pair(path, window_start, window_end)
            } else {
                (path.trades.0.clone(), HashMap::new())
            };

            possible_paths.push(self.process_paths(
                dominant_pair,
                secondary_quotes,
                fees.clone(),
                swap.swap_rate(),
                window_start,
                window_end,
            ))
        }

        let (total_taker_vwp, total_taker_volume, total_maker_vwp, total_maker_volume) =
            if exchange_cex_price.2 {
                let path = possible_paths.first().unwrap();
                (path.0.clone(), path.1.clone(), path.2.clone(), path.3.clone())
            } else {
                // ### Sort Possible Paths by Total Volume Weighted Average Price
                possible_paths.sort_by(|a, b| b.4.cmp(&a.4));
                self.sum_paths(possible_paths, swap.amount_out.clone())
            };

        let token_price = metadata
            .cex_quotes
            .get_quote_direct_or_via_intermediary(
                &Pair(swap.token_in.address, self.inner.quote),
                &exchange_cex_price.0.first().unwrap().exchange,
            )?
            .price
            .0;
        // ### Calculate Profit
        // Profit = SWAP_AMOUNT_OUT * VWAP - SWAP_AMOUNT_IN
        let taker_profit = &swap.amount_out * (total_taker_vwp / total_taker_volume)
            - &swap.amount_in * &token_price;
        let maker_profit = &swap.amount_out * (total_maker_vwp / total_maker_volume)
            - &swap.amount_in * &token_price;

        Some(ExchangeLeg {
            exchange:  exchange_cex_price.0.first().unwrap().exchange,
            cex_price: exchange_cex_price.1,
            pnl:       StatArbPnl { maker_profit, taker_profit },
            is_direct: exchange_cex_price.2,
        })
    }

    fn profit_classifier_trade_optimistic(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: Vec<(Vec<ExchangeData>, Rational, bool)>,
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        let window_start = metadata.block_timestamp - 6000;
        let window_end = metadata.block_timestamp + 6000;
        // use base vwap profit classifier to handle multiple exchanges
        // (total_taker_volume,taker_vwp,total_maker_volume,maker_vwp,total_vwap)
        let mut legs: Vec<(Rational, Rational, Rational, Rational, Rational)> = Vec::new();
        for exchange in exchange_cex_price.iter() {
            // all possible paths
            // (total_taker_volume,taker_vwp,total_maker_volume,maker_vwp,total_vwap)
            let mut possible_paths: Vec<(Rational, Rational, Rational, Rational, Rational)> =
                Vec::new();
            for path in exchange.0.iter() {
                // ### Determine Dominant Pair
                // if the path isn't direct we need to assign the dominant pair by figuring out
                // which pair has less volume ### Notes:
                // * The assumption made here is the lower volume pair is higher signal as more
                //   volume is arb related
                let (dominant_pair, secondary_quotes) = if !exchange.2 {
                    self.get_primary_pair(path, window_start, window_end)
                } else {
                    (path.trades.0.clone(), HashMap::new())
                };
                let fees = exchange.0.first().unwrap().exchange.fees();

                possible_paths.push(self.process_paths(
                    dominant_pair,
                    secondary_quotes,
                    fees,
                    swap.swap_rate(),
                    window_start,
                    window_end,
                ))
            }

            let (total_taker_vwp, total_taker_volume, total_maker_vwp, total_maker_volume) =
                if exchange.2 {
                    let path = possible_paths.first().unwrap();
                    (path.0.clone(), path.1.clone(), path.2.clone(), path.3.clone())
                } else {
                    // ### Sort Possible Paths by Total Volume Weighted Average Price
                    possible_paths.sort_by(|a, b| b.4.cmp(&a.4));
                    self.sum_paths(possible_paths, swap.amount_out.clone())
                };
            let total_vwap =
                &total_taker_vwp + &total_maker_vwp / (&total_taker_volume + &total_maker_volume);
            legs.push((
                total_maker_vwp,
                total_maker_volume,
                total_taker_vwp,
                total_taker_volume,
                total_vwap,
            ));
        }
        legs.sort_by(|a, b| b.4.cmp(&a.4));
        let (total_taker_vwp, total_taker_volume, total_maker_vwp, total_maker_volume) =
            self.sum_paths(legs, swap.amount_out.clone());

        let token_price = metadata
            .cex_quotes
            .get_quote_direct_or_via_intermediary(
                &Pair(swap.token_in.address, self.inner.quote),
                &exchange_cex_price
                    .first()
                    .unwrap()
                    .0
                    .first()
                    .unwrap()
                    .exchange,
            )?
            .price
            .0;
        // ### Calculate Profit
        // Profit = SWAP_AMOUNT_OUT * VWAP - SWAP_AMOUNT_IN
        let taker_profit = &swap.amount_out * (total_taker_vwp / total_taker_volume)
            - &swap.amount_in * &token_price;
        let maker_profit = &swap.amount_out * (total_maker_vwp / total_maker_volume)
            - &swap.amount_in * &token_price;

        Some(ExchangeLeg {
            exchange:  exchange_cex_price
                .first()
                .unwrap()
                .0
                .first()
                .unwrap()
                .exchange,
            cex_price: exchange_cex_price.first().unwrap().1.clone(),
            pnl:       StatArbPnl { maker_profit, taker_profit },
            is_direct: exchange_cex_price.first().unwrap().2,
        })
    }

    fn profit_classifier_optimistic(
        &self,
        swap: &NormalizedSwap,
        exchange_cex_price: (Vec<ExchangeData>, Rational, bool),
        metadata: &Metadata,
    ) -> Option<ExchangeLeg> {
        // Set relevant window
        let window_start = metadata.block_timestamp - 6000;
        let window_end = metadata.block_timestamp + 6000;
        let fees: (Rational, Rational) = exchange_cex_price.0.first().unwrap().exchange.fees();
        // Process trades into a vec so they can be sorted
        // (markout, volume)
        let mut raw_data: Vec<(Rational, Rational)> = Vec::new();
        let swap_rate = swap.swap_rate();
        for exchange in exchange_cex_price.0.iter() {
            let (dominant_pair, secondary_quotes) = if !exchange_cex_price.2 {
                self.get_primary_pair(exchange, window_start, window_end)
            } else {
                (exchange.trades.0.clone(), HashMap::new())
            };
            for trade in dominant_pair.iter() {
                if trade.timestamp < window_start {
                    continue;
                }
                if trade.timestamp > window_end {
                    break;
                }
                // Assign trade fee based on making or taking
                let trade_fee = match trade.side {
                    TradeSide::Buy => &fees.0,
                    _ => &fees.1,
                };
                // Calculate markout
                let mut price = &trade.price * (Rational::ONE - trade_fee);
                if !exchange_cex_price.2 {
                    price = price * &secondary_quotes.get(&trade.timestamp).unwrap().price.1;
                }
                let markout = &price - &swap_rate;

                raw_data.push((markout, trade.amount.clone()));
            }
        }

        // sort by markout with highest markouts first
        raw_data.sort_by(|a, b| b.0.cmp(&a.0));
        let mut remaining_volume = swap.amount_out.clone();
        let token_price = metadata
            .cex_quotes
            .get_quote(
                &Pair(swap.token_in.address, self.inner.quote),
                &exchange_cex_price.0.first().unwrap().exchange,
            )?
            .price
            .0;
        // Calculate total profit assuming arber got all the best markouts
        let mut total_profit = Rational::ZERO;
        for (markout, volume) in raw_data.iter() {
            // If you don't need more volume axe the rest of the makrouts and break
            if volume >= &remaining_volume {
                total_profit += markout * &remaining_volume * &token_price;
                break;
            }
            remaining_volume -= volume;
            // total profit is just markout * volume
            total_profit += markout * volume * &token_price;
        }

        Some(ExchangeLeg {
            exchange:  exchange_cex_price.0.first().unwrap().exchange,
            cex_price: exchange_cex_price.1,
            // no maker profit returned here since we don't differentiate between maker and taker
            // trades
            pnl:       StatArbPnl { maker_profit: Rational::ZERO, taker_profit: total_profit },
            is_direct: exchange_cex_price.2,
        })
    }

    /// Retrieves CEX quotes for a DEX swap, analyzing both direct and
    /// intermediary token pathways.
    ///
    /// It attempts to retrieve quotes for the pair of tokens involved in the
    /// swap from each CEX specified in the inspector's configuration. If a
    /// direct quote is unavailable for a given exchange, the function seeks
    /// a quote via an intermediary token.
    ///
    /// Direct quotes are marked as `true`, indicating a single trade. Indirect
    /// quotes are marked as `false`, indicating two trades are required to
    /// complete the swap on the CEX. This distinction is needed so we can
    /// account for CEX trading fees.
    fn cex_quotes_for_swap(
        &self,
        swap: &NormalizedSwap,
        metadata: &Metadata,
    ) -> Option<Vec<(CexExchange, Rational, bool)>> {
        let pair = Pair(swap.token_out.address, swap.token_in.address);
        let quotes = self
            .cex_exchanges
            .iter()
            .filter_map(|&exchange| {
                metadata
                    .cex_quotes
                    .get_quote(&pair, &exchange)
                    .map(|cex_quote| (exchange, cex_quote.price.0, true))
                    .or_else(|| {
                        metadata
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
            debug!("CEX quotes found for pair: {}, {} at exchanges: {:?}", pair.0, pair.1, quotes);
            Some(quotes)
        }
    }

    /// Accounts for gas costs in the calculation of potential arbitrage
    /// profits. This function calculates the final pnl for the transaction by
    /// subtracting gas costs from the total potential arbitrage profits.
    ///
    /// # Arguments
    /// * `swaps_with_profit_by_exchange` - A vector of `PossibleCexDexLeg`
    ///   instances to be analyzed.
    /// * `gas_details` - Details of the gas costs associated with the
    ///   transaction.
    /// * `metadata` - Shared metadata providing additional context and price
    ///   data.
    ///
    /// # Returns
    /// A `PossibleCexDex` instance representing the finalized arbitrage
    /// opportunity after accounting for gas costs.

    fn gas_accounting(
        &self,
        swaps_with_profit_by_exchange: Vec<PossibleCexDexLeg>,
        gas_details: &GasDetails,
        metadata: Arc<Metadata>,
    ) -> Option<PossibleCexDex> {
        let mut swaps = Vec::new();
        let mut arb_details = Vec::new();
        let mut total_arb_pre_gas = StatArbPnl::default();

        swaps_with_profit_by_exchange
            .iter()
            .for_each(|swap_with_profit| {
                if let Some(most_profitable_leg) = swap_with_profit.filter_most_profitable_leg() {
                    swaps.push(swap_with_profit.swap.clone());
                    arb_details.push(StatArbDetails {
                        cex_exchange: most_profitable_leg.exchange,
                        cex_price:    most_profitable_leg.cex_price,
                        dex_exchange: swap_with_profit.swap.protocol,
                        dex_price:    swap_with_profit.swap.swap_rate(),
                        pnl_pre_gas:  most_profitable_leg.pnl.clone(),
                    });
                    total_arb_pre_gas.maker_profit += most_profitable_leg.pnl.maker_profit;
                    total_arb_pre_gas.taker_profit += most_profitable_leg.pnl.taker_profit;
                }
            });

        if swaps.is_empty() {
            return None
        }

        let gas_cost = metadata.get_gas_price_usd(gas_details.gas_paid());

        let pnl = StatArbPnl {
            maker_profit: total_arb_pre_gas.maker_profit - gas_cost.clone(),
            taker_profit: total_arb_pre_gas.taker_profit - gas_cost,
        };

        Some(PossibleCexDex { swaps, arb_details, gas_details: *gas_details, pnl })
    }

    /// Filters and validates identified CEX-DEX arbitrage opportunities to
    /// minimize false positives.
    ///
    /// # Arguments
    /// * `possible_cex_dex` - The arbitrage opportunity being validated.
    /// * `info` - Transaction info providing additional context for validation.
    ///
    /// # Returns
    /// An option containing `BundleData::CexDex` if a valid opportunity is
    /// identified, otherwise `None`.
    fn filter_possible_cex_dex(
        &self,
        possible_cex_dex: &PossibleCexDex,
        info: &TxInfo,
        metadata: Arc<Metadata>,
    ) -> Option<BundleData> {
        if self.is_triangular_arb(possible_cex_dex, info, metadata) {
            return None
        }

        let has_positive_pnl = possible_cex_dex.pnl.maker_profit > Rational::ZERO
            || possible_cex_dex.pnl.taker_profit > Rational::ZERO;

        if has_positive_pnl
            || (!info.is_classified
                && (possible_cex_dex.gas_details.coinbase_transfer.is_some() && info.is_private
                    || info.is_cex_dex_call))
            || info.is_searcher_of_type(MevType::CexDex)
        {
            Some(possible_cex_dex.build_cex_dex_type(info))
        } else {
            None
        }
    }

    /// Filters out triangular arbitrage
    pub fn is_triangular_arb(
        &self,
        possible_cex_dex: &PossibleCexDex,
        tx_info: &TxInfo,
        metadata: Arc<Metadata>,
    ) -> bool {
        // Not enough swaps to form a cycle, thus cannot be arbitrage.
        if possible_cex_dex.swaps.len() < 2 {
            return false
        }

        let original_token = possible_cex_dex.swaps[0].token_in.address;
        let final_token = possible_cex_dex.swaps.last().unwrap().token_out.address;

        // Check if there is a cycle
        if original_token != final_token {
            return false
        }

        let profit = self
            .inner
            .get_dex_revenue_usd(
                tx_info.tx_index,
                PriceAt::Average,
                &[possible_cex_dex
                    .swaps
                    .iter()
                    .map(|s| s.to_action())
                    .collect()],
                metadata.clone(),
            )
            .unwrap_or_default();

        profit - metadata.get_gas_price_usd(tx_info.gas_details.gas_paid()) > Rational::ZERO
    }
}

pub struct PossibleCexDex {
    pub swaps:       Vec<NormalizedSwap>,
    pub arb_details: Vec<StatArbDetails>,
    pub gas_details: GasDetails,
    pub pnl:         StatArbPnl,
}

impl PossibleCexDex {
    pub fn get_swaps(&self) -> Vec<Actions> {
        self.swaps
            .iter()
            .map(|s| Actions::Swap(s.clone()))
            .collect()
    }

    pub fn build_cex_dex_type(&self, info: &TxInfo) -> BundleData {
        BundleData::CexDex(CexDex {
            tx_hash:          info.tx_hash,
            gas_details:      self.gas_details,
            swaps:            self.swaps.clone(),
            stat_arb_details: self.arb_details.clone(),
            pnl:              self.pnl.clone(),
        })
    }
}

pub struct PossibleCexDexLeg {
    pub swap:          NormalizedSwap,
    pub possible_legs: Vec<ExchangeLeg>,
}

/// Filters the most profitable exchange to execute the arbitrage on from a set
/// of potential exchanges for a given swap.
impl PossibleCexDexLeg {
    pub fn filter_most_profitable_leg(&self) -> Option<ExchangeLeg> {
        self.possible_legs
            .iter()
            .max_by_key(|leg| &leg.pnl.taker_profit)
            .cloned()
    }
}
#[derive(Clone)]
pub struct ExchangeLeg {
    pub exchange:  CexExchange,
    pub cex_price: Rational,
    pub pnl:       StatArbPnl,
    pub is_direct: bool,
}

#[cfg(test)]
mod tests {

    use alloy_primitives::hex;
    use brontes_types::constants::USDT_ADDRESS;

    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig},
        Inspectors,
    };

    #[brontes_macros::test]
    async fn test_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("21b129d221a4f169de0fc391fe0382dbde797b69300a9a68143487c54d620295").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(6772.69)
            .with_gas_paid_usd(78993.39);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_eoa_cex_dex() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("dfe3152caaf92e5a9428827ea94eff2a822ddcb22129499da4d5b6942a7f203e").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(7201.40)
            .with_gas_paid_usd(6261.08);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_triangular_arb_false_positive() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("3329c54fef27a24cef640fbb28f11d3618c63662bccc4a8c5a0d53d13267652f").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").into())
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }

    #[brontes_macros::test]
    async fn test_not_triangular_arb_false_positive_simple() {
        let inspector_util = InspectorTestUtils::new(USDT_ADDRESS, 0.5).await;

        let tx = hex!("31a1572dad67e949cff13d6ede0810678f25a30c6a3c67424453133bb822bd26").into();

        let config = InspectorTxRunConfig::new(Inspectors::CexDex)
            .with_mev_tx_hashes(vec![tx])
            .needs_token(hex!("aa7a9ca87d3694b5755f213b5d04094b8d0f0a6f").into())
            .with_dex_prices();

        inspector_util.assert_no_mev(config).await.unwrap();
    }
}
