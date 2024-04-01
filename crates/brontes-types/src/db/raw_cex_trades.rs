use std::collections::HashMap;

use alloy_primitives::Address;
use clickhouse::Row;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::{
    block_times::BlockTimes,
    cex::{CexExchange, CexPriceMap},
    cex_symbols::CexSymbols,
    cex_trades::CexTradeMap,
};
use crate::{
    db::redefined_types::primitives::*, implement_table_value_codecs_with_zc,
    serde_utils::cex_exchange,
};

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct RawCexTrades {
    #[serde(with = "cex_exchange")]
    pub exchange:  CexExchange,
    pub symbol:    String,
    pub timestamp: u64,
    pub side:      String,
    pub price:     f64,
    pub amount:    f64,
}

pub struct CexTradesConverter {
    pub block_times: Vec<BlockTimes>,
    pub symbols:     HashMap<(CexExchange, String), CexSymbols>,
    pub trades:      Vec<RawCexTrades>,
}

impl CexTradesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        trades: Vec<RawCexTrades>,
    ) -> Self {
        Self {
            block_times,
            symbols: symbols
                .into_iter()
                .map(|c| ((c.exchange.clone(), c.symbol_pair.clone()), c))
                .collect::<HashMap<_, _>>(),
            trades,
        }
    }

    pub fn convert_to_trades(self) -> Vec<(u64, CexTradeMap)> {
        /*
                let mut block_num_map = HashMap::new();

        self.quotes
            .into_par_iter()
            .filter_map(|q| {
                if let Some(block_time) = self
                    .block_times
                    .par_iter()
                    .find_any(|b| b.start_timestamp >= q.timestamp && b.end_timestamp < q.timestamp)
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

        block_num_map
            .into_par_iter()
            .map(|(block_num, quotes)| {
                let mut exchange_map = quotes
                    .iter()
                    .map(|quote| (quote.exchange, HashMap::new()))
                    .collect::<HashMap<_, _>>();

                quotes.into_par_iter().for_each(|quote| {
                    if let Some(symbol) = self.symbols.get(&(quote.exchange, quote.symbol)) {
                        exchange_map.entry(quote.exchange).or_insert(HashMap::new()).entry(symbol.address_pair)
                    }
                    //
                })

            })
            .collect()

         */
        vec![]
    }
}
