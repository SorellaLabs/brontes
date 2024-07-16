use std::collections::VecDeque;

use brontes_types::{db::cex::CexTradeMap, BlockData, FastHashMap, MultiBlockData};
use itertools::Itertools;

pub struct MultiBlockWindow {
    /// amount of blocks to hold in cache
    pub block_window_size:      usize,
    pub block_window_queue:     VecDeque<BlockData>,
    pub cex_trades_window_size: usize,
    pub cex_trades_window:      VecDeque<CexTradeMap>,
}

impl MultiBlockWindow {
    pub fn new(block_window_size: usize, cex_trades_window_size: usize) -> Self {
        Self {
            block_window_queue: VecDeque::with_capacity(block_window_size),
            block_window_size,
            cex_trades_window_size,
            cex_trades_window: VecDeque::with_capacity(cex_trades_window_size),
        }
    }

    pub fn get_cex_delta_window_mills(&self) -> usize {
        self.cex_trades_window_size
    }

    pub fn new_block_data(&mut self, data: BlockData, cex: Vec<CexTradeMap>) -> MultiBlockData {
        if self.block_window_queue.len() == self.block_window_size {
            let _ = self.block_window_queue.pop_front();
        }

        if self.cex_trades_window.len() >= self.cex_trades_window_size * 2 {
            for _ in 0..=cex.len() {
                let _ = self.cex_trades_window.pop_front();
            }
        }

        self.block_window_queue.push_back(data);
        self.cex_trades_window.extend(cex);

        let block_count = self.block_window_queue.len();
        let block_data = self.block_window_queue.clone().into_iter().collect_vec();
        let trade_data = self.cex_trades_window.clone().into_iter().fold(
            FastHashMap::<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>::default(),
            |mut acc, trades| {
                for (ex, trade) in trades.0 {
                    for (pair, trades) in trade {
                        acc.entry(ex)
                            .or_default()
                            .entry(pair)
                            .or_default()
                            .extend(trades);
                    }
                }
                acc
            },
        );

        MultiBlockData {
            blocks:         block_count,
            per_block_data: block_data,
            cex_trades:     Arc::new(trade_data),
        }
    }
}
