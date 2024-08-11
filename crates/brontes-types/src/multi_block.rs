use std::sync::Arc;

use crate::{db::metadata::Metadata, normalized_actions::Action, BlockTree};

#[derive(Debug, Clone)]
pub struct MultiBlockData {
    pub per_block_data: Vec<BlockData>,
    /// the amount of blocks in the multi block data.
    pub blocks:         usize,
}

impl MultiBlockData {
    pub fn split_to_size(&self, size: usize) -> MultiBlockData {
        let mut extra = (self.blocks as isize) - (size as isize);
        if extra < 0 {
            extra = 0
        }
        let extra = extra as usize;

        let adjusted = self
            .per_block_data
            .clone()
            .into_iter()
            .skip(extra)
            .collect::<Vec<_>>();

        Self { per_block_data: adjusted, blocks: size }
    }

    pub fn get_most_recent_block(&self) -> &BlockData {
        self.per_block_data.last().unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct BlockData {
    pub metadata: Arc<Metadata>,
    pub tree:     Arc<BlockTree<Action>>,
}

impl BlockData {
    pub fn block_number(&self) -> u64 {
        self.metadata.block_metadata.block_num
    }
}
