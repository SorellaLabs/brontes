use alloy_primitives::hex;
use clickhouse::Row;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;

use super::{CexPriceMap, CexQuote};
use crate::{
    constants::USDC_ADDRESS,
    db::{
        block_times::{BlockTimes, CexBlockTimes},
        cex::{BestCexPerPair, CexExchange, CexSymbols},
    },
    pair::Pair,
    serde_utils::cex_exchange,
    FastHashMap,
};

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct RawCexQuotes {
    #[serde(with = "cex_exchange")]
    pub exchange:   CexExchange,
    pub symbol:     String,
    pub timestamp:  u64,
    pub ask_amount: f64,
    pub ask_price:  f64,
    pub bid_price:  f64,
    pub bid_amount: f64,
}

#[derive(Debug)]
pub struct CexQuotesConverter {
    pub block_times:       Vec<CexBlockTimes>,
    pub symbols:           FastHashMap<(CexExchange, String), CexSymbols>,
    pub quotes:            Vec<RawCexQuotes>,
    pub best_cex_per_pair: Vec<BestCexPerPair>,
}

impl CexQuotesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        quotes: Vec<RawCexQuotes>,
        best_cex_per_pair: Vec<BestCexPerPair>,
    ) -> Self {
        let symbols = symbols
            .into_iter()
            .map(|c| ((c.exchange, c.symbol_pair.clone()), c))
            .collect::<FastHashMap<_, _>>();

        //TODO: Joe are you sure this won't filter out a bunch of quotes we should acc
        // be storing?
        let quotes = quotes
            .into_iter()
            .filter(|quote| symbols.contains_key(&(quote.exchange, quote.symbol.clone())))
            .collect();

        Self {
            block_times: block_times
                .into_iter()
                .map(|b| CexBlockTimes::add_time_window(b, (0.0, 300.0)))
                .sorted_by_key(|b| b.start_timestamp)
                .collect(),
            symbols,
            quotes,
            best_cex_per_pair,
        }
    }

    pub fn convert_to_prices(mut self) -> Vec<(u64, CexPriceMap)> {
        let block_num_map_with_pairs = self.create_block_num_map_with_pairs();

        block_num_map_with_pairs
            .into_iter()
            .map(|((block_num, block_time), (quotes, cex_best_venue))| {
                let exchange_map = self.group_quotes_by_exchange(quotes);
                let most_liquid_exchange_for_pair = self.process_best_cex_venues(cex_best_venue);

                let price_map = self.create_price_map(exchange_map, block_time);

                (
                    block_num,
                    CexPriceMap {
                        quotes:         price_map,
                        most_liquid_ex: most_liquid_exchange_for_pair,
                    },
                )
            })
            .collect()
    }

    fn create_price_map(
        &mut self,
        exchange_map: FastHashMap<CexExchange, Vec<RawCexQuotes>>,
        block_time: u64,
    ) -> FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>> {
        exchange_map
            .into_iter()
            .map(|(exch, quotes)| {
                let mut exchange_symbol_map: std::collections::HashMap<
                    Pair,
                    Vec<CexQuote>,
                    ahash::RandomState,
                > = FastHashMap::default();

                quotes.into_iter().for_each(|quote| {
                    let symbol = self
                        .symbols
                        .get_mut(&(quote.exchange, quote.symbol.clone()))
                        .unwrap();

                    correct_usdc_address(&mut symbol.address_pair);

                    exchange_symbol_map
                        .entry(symbol.address_pair)
                        .or_default()
                        .push(quote.into());
                });

                exchange_symbol_map =
                    find_closest_to_time_boundary(block_time, exchange_symbol_map);

                (exch, exchange_symbol_map)
            })
            .collect::<FastHashMap<_, _>>()
    }

    fn group_quotes_by_exchange(
        &self,
        quotes: Vec<RawCexQuotes>,
    ) -> FastHashMap<CexExchange, Vec<RawCexQuotes>> {
        let mut exchange_map = FastHashMap::default();
        for quote in quotes {
            exchange_map
                .entry(quote.exchange)
                .or_insert_with(Vec::new)
                .push(quote);
        }
        exchange_map
    }

    fn process_best_cex_venues(
        &mut self,
        cex_best_venue: Vec<BestCexPerPair>,
    ) -> FastHashMap<Pair, CexExchange> {
        cex_best_venue
            .into_iter()
            .filter_map(|pair_ex| {
                let symbol = self
                    .symbols
                    .get_mut(&(pair_ex.exchange, pair_ex.symbol.clone()))?;

                correct_usdc_address(&mut symbol.address_pair);

                Some((symbol.address_pair, pair_ex.exchange))
            })
            .collect()
    }

    fn create_block_num_map_with_pairs(
        &self,
    ) -> FastHashMap<(u64, u64), (Vec<RawCexQuotes>, Vec<BestCexPerPair>)> {
        let mut block_map: FastHashMap<
            (u64, u64),
            (Vec<RawCexQuotes>, FastHashMap<String, BestCexPerPair>),
        > = FastHashMap::default();

        // Process quotes
        for quote in &self.quotes {
            let matching_blocks = self.find_matching_blocks(quote.timestamp);
            for &(block_number, precise_timestamp) in &matching_blocks {
                block_map
                    .entry((block_number, precise_timestamp))
                    .or_insert_with(|| (Vec::new(), FastHashMap::default()))
                    .0
                    .push(quote.clone());
            }
        }

        // Process best_cex_per_pair
        for block_time in &self.block_times {
            let time = block_time.start_timestamp as i64;
            let entry = block_map
                .entry((block_time.block_number, block_time.precise_timestamp))
                .or_insert_with(|| (Vec::new(), FastHashMap::default()));

            for pair in &self.best_cex_per_pair {
                match entry.1.entry(pair.symbol.clone()) {
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(pair.clone());
                    }
                    std::collections::hash_map::Entry::Occupied(mut o) => {
                        let entry_time = (time - o.get().timestamp as i64).abs();
                        let this_time = (time - pair.timestamp as i64).abs();
                        if this_time < entry_time {
                            o.insert(pair.clone());
                        }
                    }
                }
            }
        }

        // Convert the FastHashMap of BestCexPerPair to Vec
        block_map
            .into_iter()
            .map(|(key, (quotes, best_pairs_map))| {
                (key, (quotes, best_pairs_map.into_values().collect()))
            })
            .collect()
    }

    fn find_matching_blocks(&self, timestamp: u64) -> Vec<(u64, u64)> {
        let mut matching_blocks = Vec::new();

        // Find the first block that contains the timestamp
        let start_idx = self
            .block_times
            .iter()
            .position(|block| block.contains_time(timestamp))
            .unwrap_or(self.block_times.len());

        // Iterate from the starting position
        for block in &self.block_times[start_idx..] {
            if block.contains_time(timestamp) || block.start_timestamp <= timestamp {
                matching_blocks.push((block.block_number, block.precise_timestamp));
            } else {
                break;
            }
        }

        matching_blocks
    }
}

const QUOTE_TIME_BOUNDARY: [u64; 6] = [0, 2, 12, 30, 60, 300];

fn find_closest_to_time_boundary(
    block_time: u64,
    exchange_symbol_map: FastHashMap<Pair, Vec<CexQuote>>,
) -> FastHashMap<Pair, Vec<CexQuote>> {
    exchange_symbol_map
        .into_par_iter()
        .filter_map(|(pair, mut quotes)| {
            if quotes.is_empty() {
                return None;
            }

            if !quotes.is_sorted_by_key(|q| q.timestamp) {
                quotes.sort_unstable_by_key(|q| q.timestamp);
            }

            let mut result = Vec::with_capacity(QUOTE_TIME_BOUNDARY.len());

            for &time in &QUOTE_TIME_BOUNDARY {
                let target_time = block_time as i128 + (time as i128 * 1_000_000);
                let idx = quotes.partition_point(|quote| quote.timestamp as i128 <= target_time);

                let closest = if idx > 0 && idx < quotes.len() {
                    let prev = &quotes[idx - 1];
                    let current = &quotes[idx];
                    if (prev.timestamp as i128 - target_time).abs()
                        <= (current.timestamp as i128 - target_time).abs()
                    {
                        prev
                    } else {
                        current
                    }
                } else if idx == quotes.len() {
                    &quotes[idx - 1]
                } else {
                    &quotes[idx]
                };

                result.push(closest.clone());
            }

            Some((pair, result))
        })
        .collect()
}

fn correct_usdc_address(pair: &mut Pair) {
    if pair.0 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        pair.0 = USDC_ADDRESS;
    } else if pair.1 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        pair.1 = USDC_ADDRESS;
    }
}
