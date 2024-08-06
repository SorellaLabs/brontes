use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use ahash::HashMapExt;
use brontes_types::{
    db::cex::{trades::CexTradeMap, CexExchange},
    pair::Pair,
    FastHashMap,
};

pub struct CexWindow {
    /// a queue of each pairs vec offset. this allows us to quickly trim
    /// out fields from the extended map
    offset_list:           VecDeque<(u64, FastHashMap<CexExchange, FastHashMap<Pair, usize>>)>,
    global_map:            Arc<RwLock<CexTradeMap>>,
    /// this is the last block loaded, adjusted for the range lookahead.
    /// this is used so that we don't double load data
    last_end_block_loaded: u64,
    window_size_seconds:   usize,
    first_block_loaded:    u64,
}

impl CexWindow {
    pub fn new(window_size_seconds: usize) -> Self {
        Self {
            offset_list: VecDeque::new(),
            global_map: Arc::new(RwLock::new(CexTradeMap::default())),
            last_end_block_loaded: 0,
            window_size_seconds,
            first_block_loaded: 0,
        }
    }

    /// used to get the initialized range going. Assumes maps are ordered
    pub fn init(&mut self, maps: Vec<(u64, CexTradeMap)>) {
        let mut global_map = self
            .global_map
            .write()
            .expect("failed to get write lock on  global cex map");

        self.first_block_loaded = maps.first().expect("No cex maps for Cex Window").0;
        self.last_end_block_loaded = maps.last().expect("No cex maps for Cex Window").0;

        for map in maps {
            let offsets = global_map.merge_in_map(map.1);
            self.offset_list.push_back((map.0, offsets));
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

    /// lets us know if the window is loaded with the necessary data.
    /// if not, we will init the full window instead of just the next block
    pub fn is_loaded(&self) -> bool {
        self.last_end_block_loaded != 0
    }

    pub fn new_block(&mut self, block: u64, new_map: CexTradeMap, active_block: u64) {
        let mut global_map = self.global_map.write().unwrap();
        let offsets = global_map.merge_in_map(new_map);
        self.offset_list.push_back((block, offsets));
        self.last_end_block_loaded = block;

        let mut accumulated_offsets = FastHashMap::new();
        let mut blocks_to_remove = 0;

        for (oldest_block, oldest_trades) in self.offset_list.iter() {
            if *oldest_block >= active_block {
                break;
            }

            blocks_to_remove += 1;

            for (ex, pairs) in oldest_trades {
                for (pair, &offset) in pairs {
                    accumulated_offsets
                        .entry(*ex)
                        .or_insert_with(FastHashMap::default)
                        .entry(*pair)
                        .and_modify(|e| *e = offset)
                        .or_insert(offset);
                }
            }
        }

        self.offset_list.drain(0..blocks_to_remove);

        if !accumulated_offsets.is_empty() {
            global_map.pop_historical_trades(accumulated_offsets);
            if let Some((first_block, _)) = self.offset_list.front() {
                self.first_block_loaded = *first_block;
            }
        }
    }

    pub fn cex_trade_map(&self) -> Arc<RwLock<CexTradeMap>> {
        Arc::clone(&self.global_map)
    }
}
