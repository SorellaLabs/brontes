use alloy_primitives::hex;
use clickhouse::Row;
use itertools::Itertools;
use serde::Deserialize;

use crate::{
    constants::USDC_ADDRESS,
    db::{
        block_times::{BlockTimes, CexBlockTimes},
        cex::{cex_symbols::CexSymbols, cex_trades::CexTradeMap, CexExchange},
    },
    serde_utils::cex_exchange,
    FastHashMap,
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
    pub symbols:     FastHashMap<String, CexSymbols>,
    pub trades:      Vec<RawCexTrades>,
}

impl CexTradesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        trades: Vec<RawCexTrades>,
        time_window: (f64, f64),
    ) -> Self {
        let symbols = symbols
            .into_iter()
            .map(|c| (c.symbol_pair.clone(), c))
            .collect::<FastHashMap<_, _>>();

        let trades = trades
            .into_iter()
            .filter(|trade| symbols.contains_key(&trade.symbol))
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
        let mut block_num_map = FastHashMap::default();

        self.trades
            .into_iter()
            .filter_map(|t| {
                self.block_times
                    .iter()
                    .find(|b| t.timestamp >= b.start_timestamp && t.timestamp < b.end_timestamp)
                    .map(|block_time| (block_time.block_number, t))
            })
            .for_each(|(block_num, trade)| {
                block_num_map
                    .entry(block_num)
                    .or_insert(Vec::new())
                    .push(trade)
            });

        block_num_map
            .into_iter()
            .map(|(block_num, trades)| {
                let mut exchange_map = FastHashMap::default();

                trades.into_iter().for_each(|trade| {
                    exchange_map
                        .entry(trade.exchange)
                        .or_insert(Vec::new())
                        .push(trade);
                });

                let cex_price_map = exchange_map
                    .into_iter()
                    .map(|(exch, trades)| {
                        let mut exchange_symbol_map = FastHashMap::default();

                        trades.into_iter().for_each(|trade| {
                            let mut symbol = self.symbols.get(&trade.symbol).unwrap().clone();

                            if symbol.address_pair.1
                                == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                            {
                                symbol.address_pair.1 = USDC_ADDRESS;
                            }

                            let pair = if &trade.side == "sell" {
                                symbol.address_pair.flip()
                            } else {
                                symbol.address_pair
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
