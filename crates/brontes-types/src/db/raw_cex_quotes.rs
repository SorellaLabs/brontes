use std::collections::HashMap;

use clickhouse::Row;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::Deserialize;

use super::{
    block_times::{BlockTimes, CexBlockTimes},
    cex::{CexExchange, CexPriceMap},
    cex_symbols::CexSymbols,
};
use crate::{serde_utils::cex_exchange, FastHashMap};

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
    pub block_times: Vec<CexBlockTimes>,
    pub symbols:     HashMap<(CexExchange, String), CexSymbols>,
    pub quotes:      Vec<RawCexQuotes>,
}

impl CexQuotesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        quotes: Vec<RawCexQuotes>,
        time_window: (u64, u64),
    ) -> Self {
        let symbols = symbols
            .into_iter()
            .map(|c| ((c.exchange, c.symbol_pair.clone()), c))
            .collect::<HashMap<_, _>>();

        let quotes = quotes
            .into_iter()
            .filter(|quote| symbols.contains_key(&(quote.exchange, quote.symbol.clone())))
            .collect();

        Self {
            block_times: block_times
                .into_iter()
                .map(|b| CexBlockTimes::add_time_window(b, time_window))
                .sorted_by_key(|b| b.start_timestamp)
                .collect(),
            symbols,
            quotes,
        }
    }

    pub fn convert_to_prices(self) -> Vec<(u64, CexPriceMap)> {
        let mut block_num_map = HashMap::new();

        self.quotes
            .into_par_iter()
            .filter_map(|q| {
                self.block_times
                    .par_iter()
                    .find_any(|b| q.timestamp >= b.start_timestamp && q.timestamp < b.end_timestamp)
                    .map(|block_time| (block_time.block_number, q))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|(block_num, quote)| {
                block_num_map
                    .entry(block_num)
                    .or_insert(Vec::new())
                    .push(quote)
            });

        block_num_map
            .into_par_iter()
            .map(|(block_num, quotes)| {
                let mut exchange_map = HashMap::new();

                quotes.into_iter().for_each(|quote| {
                    exchange_map
                        .entry(quote.exchange)
                        .or_insert(Vec::new())
                        .push(quote);
                });

                let cex_price_map = exchange_map
                    .into_par_iter()
                    .map(|(exch, quotes)| {
                        let mut exchange_symbol_map = HashMap::new();

                        quotes.into_iter().for_each(|quote| {
                            let symbol = self
                                .symbols
                                .get(&(quote.exchange, quote.symbol.clone()))
                                .unwrap();
                            exchange_symbol_map
                                .entry(&symbol.address_pair)
                                .or_insert(Vec::new())
                                .push(quote);
                        });

                        let symbol_price_map = exchange_symbol_map
                            .into_par_iter()
                            .map(|(pair, quotes)| {
                                let best_quote =
                                    quotes.into_par_iter().max_by_key(|q| q.timestamp).unwrap();
                                let pair_quote = (*pair, best_quote);

                                (pair.ordered(), pair_quote.into())
                            })
                            .collect::<FastHashMap<_, _>>();

                        (exch, symbol_price_map)
                    })
                    .collect::<FastHashMap<_, _>>();

                (block_num, CexPriceMap(cex_price_map))
            })
            .collect()
    }
}

/*

Price delta between CEX 'Okex' with price '0.00002568333603681798' and DEX 'UniswapV3' with price '38936.70962745134' for token in '("USDC", 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48)' and token out '("WBTC", 0x2260fac5e5542a773aa44fbcfedf7c1Processing blocks:

cargo test --package brontes-inspect --lib --features cex-dex-markout,local-reth,local-clickhouse -- mev_inspectors::cex_dex_markout::tests --nocapture




Price delta between CEX 'Okex' with price '0.000025682512777050106' and DEX 'UniswapV3' with price '38929.74478611938' for




IN MAP: (0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48, 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599) -> inverse price


token in '("USDC", 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48)'
token out '("WBTC", 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599)'
-> check token0

token in '("WBTC", 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599)'
token out '("USDC", 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48)'
-> check token0







IN MAP: (0x3472a5a71965499acd81997a54bba8d852c6e53d, 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599) -> inverse price


token in '("BADGER", 0x3472a5a71965499acd81997a54bba8d852c6e53d)'
token out '("WBTC", 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599)'
-> check token0






IN MAP: (0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48, 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599) -> price


token in '("WBTC", 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599)'
token out '("USDC", 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48)'






*/
