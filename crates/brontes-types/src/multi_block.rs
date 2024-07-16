use std::sync::Arc;

use crate::{
    db::{cex::CexTradeMap, metadata::Metadata},
    normalized_actions::Action,
    BlockTree,
};

#[derive(Debug, Clone)]
pub struct MultiBlockData {
    pub per_block_data: Vec<BlockData>,
    /// the amount of blocks in the multi block data.
    pub blocks:         usize,
}

impl MultiBlockData {
    pub fn split_to_size(&self, size: usize) -> MultiBlockData {
        let extra = self.blocks - size;
        let adjusted = self
            .per_block_data
            .clone()
            .into_iter()
            .skip(extra)
            .collect::<Vec<_>>();

        Self { adjusted, blocks: size }
    }
}

#[derive(Debug, Clone)]
pub struct BlockData {
    pub metadata: Arc<Metadata>,
    pub tree:     Arc<BlockTree<Action>>,
}
