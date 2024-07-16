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

#[derive(Debug, Clone)]
pub struct BlockData {
    pub metadata:   Arc<Metadata>,
    pub tree:       Arc<BlockTree<Action>>,
}
