use std::collections::HashMap;

use alloy_primitives::hex;
use clickhouse::Row;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::Deserialize;

use super::{
    block_times::BlockTimes, cex::CexExchange, cex_symbols::CexSymbols, cex_trades::CexTradeMap,
};
use crate::{
    constants::USDC_ADDRESS, db::block_times::CexBlockTimes, serde_utils::cex_exchange, FastHashMap,
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
    pub block_times: Vec<CexBlockTimes>,
    pub symbols:     HashMap<(CexExchange, String), CexSymbols>,
    pub trades:      Vec<RawCexTrades>,
}

impl CexTradesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        trades: Vec<RawCexTrades>,
        time_window: (u64, u64),
    ) -> Self {
        let symbols = symbols
            .into_iter()
            .map(|c| ((c.exchange, c.symbol_pair.clone()), c))
            .collect::<HashMap<_, _>>();

        let trades = trades
            .into_iter()
            .filter(|trade| symbols.contains_key(&(trade.exchange, trade.symbol.clone())))
            .collect();

        Self {
            block_times: block_times
                .into_iter()
                .map(|b| CexBlockTimes::add_time_window(b, time_window))
                .sorted_by_key(|b| b.start_timestamp)
                .collect(),
            symbols,
            trades,
        }
    }

    pub fn convert_to_trades(self) -> Vec<(u64, CexTradeMap)> {
        let mut block_num_map = HashMap::new();

        self.trades
            .into_par_iter()
            .filter_map(|q| {
                self.block_times
                    .par_iter()
                    .find_any(|b| q.timestamp >= b.start_timestamp && q.timestamp < b.end_timestamp)
                    .map(|block_time| (block_time.block_number, q))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|(block_num, trade)| {
                block_num_map
                    .entry(block_num)
                    .or_insert(Vec::new())
                    .push(trade)
            });

        block_num_map
            .into_par_iter()
            .map(|(block_num, trades)| {
                let mut exchange_map = HashMap::new();

                trades.into_iter().for_each(|trade| {
                    exchange_map
                        .entry(trade.exchange)
                        .or_insert(Vec::new())
                        .push(trade);
                });

                let cex_price_map = exchange_map
                    .into_par_iter()
                    .map(|(exch, trades)| {
                        let mut exchange_symbol_map = FastHashMap::default();

                        trades.into_iter().for_each(|mut trade| {
                            let mut symbol = self
                                .symbols
                                .get(&(trade.exchange, trade.symbol.clone()))
                                .unwrap();
                            if symbol.address_pair.1
                                == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                            {
                                symbol.address_pair.1 = USDC_ADDRESS;
                            }

                            let pair = if &trade.side == "buy" {
                                symbol.address_pair
                            } else {
                                trade.amount *= trade.price;
                                trade.price = 1.0 / trade.price;
                                symbol.address_pair.flip()
                            };

                            exchange_symbol_map
                                .entry(pair)
                                .or_insert(Vec::new())
                                .push(trade.into());
                        });

                        (exch, exchange_symbol_map)
                    })
                    .collect::<FastHashMap<_, _>>();

                (block_num, CexTradeMap(cex_price_map))
            })
            .collect()
    }
}
