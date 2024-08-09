use std::mem;

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
                let most_liquid_exchange_for_pair = self.process_best_cex_venues(cex_best_venue);

                let price_map = self.create_price_map(quotes, block_time);

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

    pub fn create_price_map(
        &mut self,
        exchange_map: FastHashMap<CexExchange, Vec<usize>>,
        block_time: u64,
    ) -> FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>> {
        exchange_map
            .into_iter()
            .map(|(exch, quote_indices)| {
                let mut exchange_pair_index_map: std::collections::HashMap<
                    Pair,
                    Vec<usize>,
                    ahash::RandomState,
                > = FastHashMap::default();

                quote_indices.into_iter().for_each(|index| {
                    let quote = &self.quotes[index];

                    let symbol = self
                        .symbols
                        .get_mut(&(quote.exchange, quote.symbol.clone()))
                        .unwrap();

                    correct_usdc_address(&mut symbol.address_pair);

                    exchange_pair_index_map
                        .entry(symbol.address_pair)
                        .or_default()
                        .push(index);
                });

                let exchange_symbol_map =
                    self.find_closest_to_time_boundary(block_time, exchange_pair_index_map);

                (exch, exchange_symbol_map)
            })
            .collect::<FastHashMap<_, _>>()
    }

    pub fn process_best_cex_venues(
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

    pub fn create_block_num_map_with_pairs(
        &self,
    ) -> FastHashMap<(u64, u64), (FastHashMap<CexExchange, Vec<usize>>, Vec<BestCexPerPair>)> {
        let mut block_map: FastHashMap<
            (u64, u64),
            (FastHashMap<CexExchange, Vec<usize>>, FastHashMap<String, BestCexPerPair>),
        > = FastHashMap::default();

        // Process quotes
        for (index, quote) in self.quotes.iter().enumerate() {
            let matching_blocks = self.find_matching_blocks(quote.timestamp);
            let exchange = quote.exchange;
            for &(block_number, precise_timestamp) in &matching_blocks {
                block_map
                    .entry((block_number, precise_timestamp))
                    .or_default()
                    .0
                    .entry(exchange)
                    .or_default()
                    .push(index);
            }
        }

        // Process best_cex_per_pair
        for block_time in &self.block_times {
            let time = block_time.start_timestamp as i64;
            let entry = block_map
                .entry((block_time.block_number, block_time.precise_timestamp))
                .or_default();

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

    pub fn find_matching_blocks(&self, timestamp: u64) -> Vec<(u64, u64)> {
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

    pub fn find_closest_to_time_boundary(
        &self,
        block_time: u64,
        exchange_symbol_map: FastHashMap<Pair, Vec<usize>>,
    ) -> FastHashMap<Pair, Vec<CexQuote>> {
        exchange_symbol_map
            .into_par_iter()
            .filter_map(|(pair, quotes_indices)| {
                if quotes_indices.is_empty() {
                    return None;
                }

                let mut result = Vec::with_capacity(QUOTE_TIME_BOUNDARY.len());

                for &time in &QUOTE_TIME_BOUNDARY {
                    let target_time = block_time as i128 + (time as i128 * 1_000_000);
                    let quote_index = quotes_indices.first().unwrap();

                    let closest = if quote_index > &0 && quote_index < &quotes_indices.len() {
                        let prev = &self.quotes[*quote_index - 1];
                        let current = &self.quotes[*quote_index];
                        if (prev.timestamp as i128 - target_time).abs()
                            <= (current.timestamp as i128 - target_time).abs()
                        {
                            prev
                        } else {
                            current
                        }
                    } else if *quote_index == quotes_indices.len() {
                        &self.quotes[*quote_index - 1]
                    } else {
                        &self.quotes[*quote_index]
                    };

                    result.push(closest.clone().into());
                }

                Some((pair, result))
            })
            .collect()
    }
}

const QUOTE_TIME_BOUNDARY: [u64; 6] = [0, 2, 12, 30, 60, 300];

pub fn correct_usdc_address(pair: &mut Pair) {
    if pair.0 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        pair.0 = USDC_ADDRESS;
    } else if pair.1 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        pair.1 = USDC_ADDRESS;
    }
}

#[allow(unused)]
pub fn approximate_size_of_converter(converter: &CexQuotesConverter) -> usize {
    let mut total_size = mem::size_of_val(converter);

    total_size += mem::size_of_val(&converter.block_times);
    total_size += converter.block_times.len() * mem::size_of::<CexBlockTimes>();

    total_size += mem::size_of_val(&converter.symbols);
    for ((exchange, symbol), cex_symbols) in &converter.symbols {
        total_size += mem::size_of_val(exchange);
        total_size += symbol.capacity();
        total_size += size_of_cex_symbols(cex_symbols);
    }

    total_size += mem::size_of_val(&converter.quotes);
    total_size += converter
        .quotes
        .iter()
        .map(size_of_raw_cex_quotes)
        .sum::<usize>();

    // Size of best_cex_per_pair
    total_size += mem::size_of_val(&converter.best_cex_per_pair);
    total_size += converter
        .best_cex_per_pair
        .iter()
        .map(size_of_best_cex_per_pair)
        .sum::<usize>();

    total_size
}

fn size_of_cex_symbols(symbols: &CexSymbols) -> usize {
    mem::size_of_val(symbols) + symbols.symbol_pair.capacity() + mem::size_of::<Pair>()
}

fn size_of_raw_cex_quotes(quotes: &RawCexQuotes) -> usize {
    mem::size_of_val(quotes) + quotes.symbol.capacity()
}

fn size_of_best_cex_per_pair(best_cex: &BestCexPerPair) -> usize {
    mem::size_of_val(best_cex) + best_cex.symbol.capacity()
}
