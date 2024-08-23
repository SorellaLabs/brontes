use alloy_primitives::{hex, Address};
use clickhouse::Row;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;
use strum::Display;

use crate::{
    constants::USDC_ADDRESS,
    db::{
        block_times::{BlockTimes, CexBlockTimes},
        cex::{cex_symbols::CexSymbols, trades::CexTradeMap, CexExchange},
    },
    execute_on,
    serde_utils::{cex_exchange, trade_type},
    FastHashMap,
};

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct RawCexTrades {
    #[serde(with = "cex_exchange")]
    pub exchange:   CexExchange,
    #[serde(with = "trade_type")]
    pub trade_type: TradeType,
    pub symbol:     String,
    pub timestamp:  u64,
    pub side:       String,
    pub price:      f64,
    pub amount:     f64,
}

#[derive(
    Debug,
    Clone,
    Display,
    PartialEq,
    Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Default,
)]
pub enum TradeType {
    Maker,
    #[default]
    Taker,
}

pub struct CexTradesConverter {
    pub block_times: Vec<CexBlockTimes>,
    pub symbols:     FastHashMap<String, Vec<CexSymbols>>,
    pub trades:      Vec<RawCexTrades>,
}

impl CexTradesConverter {
    pub fn new(
        block_times: Vec<BlockTimes>,
        symbols: Vec<CexSymbols>,
        trades: Vec<RawCexTrades>,
    ) -> Self {
        let symbols = symbols.into_iter().fold(
            FastHashMap::<String, Vec<CexSymbols>>::default(),
            |mut acc, x| {
                acc.entry(x.symbol_pair.clone()).or_default().push(x);
                acc
            },
        );

        let trades = trades
            .into_iter()
            .filter(|trade| symbols.contains_key(&trade.symbol))
            .collect();

        Self {
            block_times: block_times
                .into_iter()
                .map(|b| CexBlockTimes::add_time_window(b, (6.0, 6.0)))
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
                    .find(|b| b.contains_time(t.timestamp))
                    .map(|block_time| (block_time.block_number, t))
            })
            .for_each(|(block_num, trade)| {
                block_num_map
                    .entry(block_num)
                    .or_insert(Vec::new())
                    .push(trade)
            });

        execute_on!(download, {
            block_num_map
                .into_par_iter()
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
                                let symbols = self.symbols.get(&trade.symbol).unwrap().clone();

                                // there is a case were we have multiple addresses for
                                // same symbol so this covers it.
                                let mut seen = vec![];
                                for mut symbol in symbols {
                                    if seen.contains(&symbol.address_pair) {
                                        continue
                                    } else {
                                        seen.push(symbol.address_pair)
                                    }

                                    if symbol.address_pair.1
                                        == hex!("2f6081e3552b1c86ce4479b80062a1dda8ef23e3")
                                    {
                                        symbol.address_pair.1 = USDC_ADDRESS;
                                    }

                                    if symbol.address_pair.0
                                        == hex!("15D4c048F83bd7e37d49eA4C83a07267Ec4203dA")
                                        && trade.timestamp > 1684220400000000
                                    {
                                        symbol.address_pair.0 = Address::from(hex!(
                                            "d1d2Eb1B1e90B638588728b4130137D262C87cae"
                                        ))
                                    }

                                    exchange_symbol_map
                                        .entry(symbol.address_pair)
                                        .or_insert(Vec::new())
                                        .push(trade.clone().into());
                                }
                            });

                            (exch, exchange_symbol_map)
                        })
                        .collect::<FastHashMap<_, _>>();

                    (block_num, CexTradeMap(cex_price_map))
                })
                .collect()
        })
    }
}
