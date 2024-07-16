use std::collections::VecDeque;

use brontes_types::{db::cex::CexTradeMap, BlockData, MultiBlockData};
use itertools::Itertools;

pub struct MultiBlockWindow {
    /// amount of blocks to hold in cache
    pub block_window_size:  usize,
    pub block_window_queue: VecDeque<BlockData>,
}

impl MultiBlockWindow {
    pub fn new(block_window_size: usize) -> Self {
        Self { block_window_queue: VecDeque::with_capacity(block_window_size), block_window_size }
    }

    pub fn new_block_data(&mut self, data: BlockData) -> MultiBlockData {
        if self.block_window_queue.len() == self.block_window_size {
            let _ = self.block_window_queue.pop_front();
        }

        self.block_window_queue.push_back(data);

        let block_count = self.block_window_queue.len();
        let block_data = self.block_window_queue.clone().into_iter().collect_vec();

        MultiBlockData { blocks: block_count, per_block_data: block_data }
    }
}
