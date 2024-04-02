use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{
    block_times::{BlockTimes, CexBlockTimes},
    cex::{CexExchange, CexPriceMap},
    cex_symbols::CexSymbols,
};
use crate::{
    db::redefined_types::primitives::*, implement_table_value_codecs_with_zc,
    serde_utils::cex_exchange, FastHashMap,
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
    pub block_times: Vec<CexBlockTimes>,
    pub symbols:     HashMap<(CexExchange, String), CexSymbols>,
    pub quotes:      Vec<RawCexQuotes>,
}

impl CexQuotesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        quotes: Vec<RawCexQuotes>,
    ) -> Self {
        println!(
            "\nEXCHANGES PRE: {:?}\n",
            quotes
                .iter()
                .map(|q| q.exchange)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect_vec()
        );

        let symbols = symbols
            .into_iter()
            .map(|c| ((c.exchange.clone(), c.symbol_pair.clone()), c))
            .collect::<HashMap<_, _>>();

        let quotes = quotes
            .into_iter()
            .filter(|quote| {
                symbols
                    .get(&(quote.exchange, quote.symbol.clone()))
                    .is_some()
            })
            .collect();

        Self {
            block_times: block_times
                .into_iter()
                .map(CexBlockTimes::quote_times)
                .sorted_by_key(|b| b.start_timestamp)
                .collect(),
            symbols,
            quotes,
        }
    }

    pub fn convert_to_prices(self) -> Vec<(u64, CexPriceMap)> {
        let mut block_num_map = HashMap::new();

        println!("\nBLOCK TIMES: {:?}\n", self.block_times);
        println!("\nQUOTES: {:?}\n", self.quotes.len());

        println!(
            "\nEXCHANGES POST: {:?}\n",
            self.quotes
                .iter()
                .map(|q| q.exchange)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect_vec()
        );

        self.quotes
            .into_par_iter()
            .filter_map(|q| {
                if let Some(block_time) = self
                    .block_times
                    .par_iter()
                    .find_any(|b| q.timestamp >= b.start_timestamp && q.timestamp < b.end_timestamp)
                {
                    Some((block_time.block_number, q))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|(block_num, quote)| {
                block_num_map
                    .entry(block_num)
                    .or_insert(Vec::new())
                    .push(quote)
            });

        // println!("\nBLOCK MAP: {:?}\n", block_num_map);

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

                                (*pair, pair_quote.into())
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
