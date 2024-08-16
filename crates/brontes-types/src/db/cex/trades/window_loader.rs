use std::collections::VecDeque;

use crate::{
    db::cex::{trades::CexTradeMap, CexExchange},
    pair::Pair,
    FastHashMap,
};

pub struct CexWindow {
    /// a queue of each pairs vec offset. this allows us to quickly trim
    /// out fields from the extended mapw
    offset_list:           VecDeque<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    global_map:            CexTradeMap,
    /// this is the last block loaded, adjusted for the range lookahead.
    /// this is used so that we don't double load data
    last_end_block_loaded: u64,
    window_size_seconds:   usize,
}

impl CexWindow {
    pub fn new(window_size_seconds: usize) -> Self {
        Self {
            offset_list: VecDeque::new(),
            global_map: CexTradeMap::default(),
            last_end_block_loaded: 0,
            window_size_seconds,
        }
    }

    /// used to get the initialized range going. Assumes maps are ordered
    pub fn init(&mut self, end_block: u64, maps: Vec<CexTradeMap>) {
        self.last_end_block_loaded = end_block;
        for map in maps {
            let offsets = self.global_map.merge_in_map(map);
            self.offset_list.push_back(offsets);
        }
    }

    pub fn get_last_end_block_loaded(&self) -> u64 {
        self.last_end_block_loaded
    }

    pub fn get_window_lookahead(&self) -> usize {
        self.window_size_seconds
    }

    pub fn set_last_block(&mut self, block: u64) {
        self.last_end_block_loaded = block;
    }

    /// lets us know if the window is loaded with the nessacary data.
    /// if not, we will init the full window instead of just the next block
    pub fn is_loaded(&self) -> bool {
        self.last_end_block_loaded != 0
    }

    pub fn new_block(&mut self, new_map: CexTradeMap) {
        let offsets = self.global_map.merge_in_map(new_map);
        self.offset_list.push_back(offsets);

        let Some(oldest_trades) = self.offset_list.pop_front() else { return };
        self.global_map.pop_historical_trades(oldest_trades);
    }

    pub fn cex_trade_map(&self) -> CexTradeMap {
        // we gotta clone or else we get a race condition where when
        // we remove old data. processing might still be occurring thus shifting
        // the time window from whats expected.
        self.global_map.clone()
    }
}
