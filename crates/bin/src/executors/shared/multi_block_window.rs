use std::collections::VecDeque;

use brontes_types::{BlockData, MultiBlockData};

pub struct MultiBlockWindow {
    pub block_window_size:      usize,
    pub block_window_queue:     VecDeque<BlockData>,
    pub cex_trades_window_size: usize,
    pub cex_trades_window:      VecDeque<()>,
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

    pub fn new_block_data(&mut self, data: BlockData, cex: ()) -> MultiBlockData {
        todo!()
    }
}
