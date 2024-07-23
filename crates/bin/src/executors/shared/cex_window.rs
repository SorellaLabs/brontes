use std::collections::VecDeque;

use brontes_types::{
    db::cex::{CexExchange, CexTradeMap},
    pair::Pair,
    FastHashMap,
};

pub struct CexWindow {
    /// a queue of each pairs vec offset. this allows us to quickly trim
    /// out fields from the extended map
    offset_list:            VecDeque<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    global_map:             CexTradeMap,
    window_size_blocks:     usize,
    window_block_lookahead: usize,
}

impl CexWindow {
    pub fn new(window_size_blocks: usize, window_block_lookahead: usize) -> Self {
        Self {
            offset_list: VecDeque::new(),
            global_map: CexTradeMap::default(),
            window_size_blocks,
            window_block_lookahead,
        }
    }

    pub fn get_window_size(&self) -> usize {
        self.window_size_blocks
    }

    pub fn new_block(&mut self, new_map: CexTradeMap) {
        let offsets = self.global_map.merge_in_map(new_map);
        self.offset_list.push_back(offsets);

        if self.offset_list.len() > self.window_size_blocks {
            let Some(oldest_trades) = self.offset_list.pop_front() else { return };
            self.global_map.pop_historical_trades(oldest_trades);
        }
    }

    pub fn cex_trade_map(&self) -> CexTradeMap {
        // we gotta clone or else we get a race condition where when
        // we remove old data. processing might still be occurring thus shifting
        // the time window from the expected value
        self.global_map.clone()
    }
}
