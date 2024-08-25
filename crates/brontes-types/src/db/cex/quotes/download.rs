use std::mem;

use alloy_primitives::{hex, Address};
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
    execute_on,
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
                .map(|b| CexBlockTimes::add_time_window(b, (301.0, 301.0)))
                .sorted_by_key(|b| b.start_timestamp)
                .collect(),
            symbols,
            quotes,
            best_cex_per_pair,
        }
    }

    pub fn convert_to_prices(self) -> Vec<(u64, CexPriceMap)> {
        let block_num_map_with_pairs = self.create_block_num_map_with_pairs();

        let most_liquid_exchange_for_pair = &self.process_best_cex_venues();

        execute_on!(download, {
            block_num_map_with_pairs
                .into_par_iter()
                .map(|((block_num, block_time), quotes)| {
                    let price_map = self.create_price_map(quotes, block_time);
                    (
                        block_num,
                        CexPriceMap {
                            quotes:         price_map,
                            most_liquid_ex: most_liquid_exchange_for_pair.clone(),
                        },
                    )
                })
                .collect()
        })
    }

    pub fn create_price_map(
        &self,
        exchange_map: FastHashMap<CexExchange, Vec<usize>>,
        block_time: u64,
    ) -> FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexQuote>>> {
        exchange_map
            .into_iter()
            .map(|(exch, quote_indices)| {
                let mut exchange_pair_index_map: FastHashMap<Pair, Vec<usize>> =
                    FastHashMap::default();

                quote_indices.into_iter().for_each(|index| {
                    let quote = &self.quotes[index];

                    let symbol = self
                        .symbols
                        .get(&(quote.exchange, quote.symbol.clone()))
                        .unwrap();

                    let mut pair = correct_usdc_address(&symbol.address_pair);

                    if symbol.address_pair.0 == hex!("15D4c048F83bd7e37d49eA4C83a07267Ec4203dA")
                        && quote.timestamp > 1684220400000000
                    {
                        pair.0 = Address::from(hex!("d1d2Eb1B1e90B638588728b4130137D262C87cae"))
                    }

                    exchange_pair_index_map.entry(pair).or_default().push(index);
                });

                let exchange_symbol_map =
                    self.find_closest_to_time_boundary(block_time, exchange_pair_index_map);

                (exch, exchange_symbol_map)
            })
            .collect::<FastHashMap<_, _>>()
    }

    pub fn process_best_cex_venues(&self) -> FastHashMap<Pair, Vec<CexExchange>> {
        self.best_cex_per_pair
            .iter()
            .filter_map(|pair_ex| {
                let symbol = pair_ex
                    .exchange
                    .iter()
                    .find_map(|exchange| self.symbols.get(&(*exchange, pair_ex.symbol.clone())))?;

                let pair = correct_usdc_address(&symbol.address_pair);

                Some((pair, pair_ex.clone().exchange))
            })
            .collect()
    }

    pub fn create_block_num_map_with_pairs(
        &self,
    ) -> FastHashMap<(u64, u64), FastHashMap<CexExchange, Vec<usize>>> {
        let mut block_map: FastHashMap<(u64, u64), FastHashMap<CexExchange, Vec<usize>>> =
            FastHashMap::default();

        // Process quotes
        let mut last_block = 0;

        for (index, quote) in self.quotes.iter().enumerate() {
            let (matching_blocks, latest_block) =
                self.find_matching_blocks(quote.timestamp, last_block);
            last_block = latest_block;

            let exchange = quote.exchange;
            for &(block_number, precise_timestamp) in &matching_blocks {
                block_map
                    .entry((block_number, precise_timestamp))
                    .or_default()
                    .entry(exchange)
                    .or_default()
                    .push(index);
            }
        }

        block_map
    }

    pub fn find_matching_blocks(
        &self,
        timestamp: u64,
        last_block: usize,
    ) -> (Vec<(u64, u64)>, usize) {
        let mut matching_blocks = Vec::with_capacity(25);
        let len = self.block_times.len();

        // Find the first block that contains the timestamp
        let start_idx = if last_block != 0 {
            if self.block_times[last_block].contains_time(timestamp) {
                matching_blocks.push((
                    self.block_times[last_block].block_number,
                    self.block_times[last_block].precise_timestamp,
                ));
                last_block + 1
            } else {
                last_block
            }
        } else {
            self.block_times
                .iter()
                .position(|block| block.contains_time(timestamp))
                .unwrap_or(self.block_times.len())
        };

        let end_idx = len.min(start_idx + 27);

        for block in &self.block_times[start_idx..end_idx] {
            if block.contains_time(timestamp) {
                matching_blocks.push((block.block_number, block.precise_timestamp));
            } else {
                break
            }
        }

        (matching_blocks, last_block.saturating_sub(1))
    }

    pub fn find_closest_to_time_boundary(
        &self,
        block_time: u64,
        exchange_symbol_map: FastHashMap<Pair, Vec<usize>>,
    ) -> FastHashMap<Pair, Vec<CexQuote>> {
        execute_on!(download, {
            exchange_symbol_map
                .into_par_iter()
                .filter_map(|(pair, quotes_indices)| {
                    if quotes_indices.is_empty() {
                        return None
                    }

                    let mut result = Vec::with_capacity(QUOTE_TIME_BOUNDARY.len());

                    let mut quote_iter = quotes_indices
                        .into_iter()
                        .map(|q| self.quotes[q].clone())
                        .peekable();

                    let mut last_quote = quote_iter.next();

                    for time in QUOTE_TIME_BOUNDARY {
                        while let Some(peeked) = quote_iter.peek() {
                            if peeked.timestamp > block_time + time * 1_000_000 {
                                break
                            }
                            last_quote = quote_iter.next();
                        }

                        if let Some(quote) = &last_quote {
                            result.push(quote.clone().into());
                        }
                    }

                    Some((pair, result))
                })
                .collect()
        })
    }
}

const QUOTE_TIME_BOUNDARY: [u64; 6] = [0, 2, 12, 30, 60, 300];

pub fn correct_usdc_address(pair: &Pair) -> Pair {
    let mut corrected_pair = *pair;
    if corrected_pair.0 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        corrected_pair.0 = USDC_ADDRESS;
    } else if corrected_pair.1 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3") {
        corrected_pair.1 = USDC_ADDRESS;
    }
    corrected_pair
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
