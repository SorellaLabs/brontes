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
        let block_num_map_with_pairs = self
            .quotes
            .into_iter()
            .filter_map(|q| {
                self.block_times
                    .iter()
                    .find(|b| b.contains_time(q.timestamp))
                    .map(|block_time| (block_time, q))
            })
            .map(|(block_time, quote)| (block_time, quote))
            .into_group_map()
            .into_iter()
            .map(|(block_time, quotes)| {
                let time = block_time.start_timestamp as i64;
                // we want to choose the pairs that are closest timestamp
                let mut map = FastHashMap::default();
                self.best_cex_per_pair.iter().for_each(|pair| {
                    match map.entry(pair.symbol.clone()) {
                        std::collections::hash_map::Entry::Vacant(v) => {
                            v.insert(pair);
                        }
                        std::collections::hash_map::Entry::Occupied(mut o) => {
                            let entry = o.get();
                            let entry_time = (time - entry.timestamp as i64).abs();
                            let this_time = (time - pair.timestamp as i64).abs();
                            if this_time < entry_time {
                                o.insert(pair);
                            }
                        }
                    }
                });
                (
                    (block_time.block_number, block_time.precise_timestamp),
                    (quotes, map.into_values().cloned().collect::<Vec<_>>()),
                )
            })
            .collect::<FastHashMap<_, _>>();

        block_num_map_with_pairs
            .into_iter()
            .map(|((block_num, block_time), (quotes, cex_best_venue))| {
                let mut exchange_map = FastHashMap::default();

                quotes.into_iter().for_each(|quote| {
                    exchange_map
                        .entry(quote.exchange)
                        .or_insert(Vec::new())
                        .push(quote);
                });

                let pair_exchanges = cex_best_venue
                    .into_iter()
                    .filter_map(|pair_ex| {
                        let symbol = self
                            .symbols
                            .get_mut(&(pair_ex.exchange, pair_ex.symbol.clone()))?;

                        //TODO: Joe, please fix USDC to not be dollar lmao
                        if symbol.address_pair.1 == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                        {
                            symbol.address_pair.1 = USDC_ADDRESS;
                        } else if symbol.address_pair.0
                            == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                        {
                            symbol.address_pair.0 = USDC_ADDRESS;
                        }
                        Some((symbol.address_pair, pair_ex.exchange))
                        // because we know there will only be 1 entry per
                        // address pair. this is ok todo
                    })
                    .collect::<FastHashMap<_, _>>();

                let cex_price_map = exchange_map
                    .into_iter()
                    .map(|(exch, quotes)| {
                        let mut exchange_symbol_map = FastHashMap::default();

                        quotes.into_iter().for_each(|quote| {
                            let symbol = self
                                .symbols
                                .get_mut(&(quote.exchange, quote.symbol.clone()))
                                .unwrap();

                            //TODO: Joe, please fix USDC to not be dollar lmao
                            if symbol.address_pair.1
                                == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                            {
                                symbol.address_pair.1 = USDC_ADDRESS;
                            } else if symbol.address_pair.0
                                == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                            {
                                symbol.address_pair.0 = USDC_ADDRESS;
                            }

                            exchange_symbol_map
                                .entry(symbol.address_pair)
                                .or_insert(Vec::new())
                                .push(quote.into());
                        });

                        exchange_symbol_map =
                            find_closest_to_time_boundries(block_time, exchange_symbol_map);

                        for quotes in exchange_symbol_map.values_mut() {
                            if !quotes.is_sorted_by_key(|k: &CexQuote| k.timestamp) {
                                quotes.sort_unstable_by_key(|k: &CexQuote| k.timestamp);
                            }
                        }

                        (exch, exchange_symbol_map)
                    })
                    .collect::<FastHashMap<_, _>>();

                (block_num, CexPriceMap { quotes: cex_price_map, most_liquid_ex: pair_exchanges })
            })
            .collect()
    }
}

const QUOTE_TIME_BOUNDARY: [u64; 6] = [0, 2, 12, 30, 60, 300];
fn find_closest_to_time_boundries(
    block_time: u64,
    exchange_symbol_map: FastHashMap<Pair, Vec<CexQuote>>,
) -> FastHashMap<Pair, Vec<CexQuote>> {
    let block_time = block_time as u128 * 1000000;
    exchange_symbol_map
        .into_par_iter()
        .map(|(pair, quotes)| {
            (
                pair,
                QUOTE_TIME_BOUNDARY
                    .iter()
                    .filter_map(|window| {
                        quotes.iter().min_by_key(|quote| {
                            let delta = quote.timestamp as i128 - block_time as i128;
                            let window = *window as i128 * 1000000;
                            (delta - window as i128).abs()
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<FastHashMap<Pair, Vec<CexQuote>>>()
}
