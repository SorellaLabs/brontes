use crate::{
    db::{cex::CexTradeMap, metadata::Metadata},
    normalized_actions::Action,
    BlockTree,
};

pub struct MultiBlockData {
    pub per_block_data: Vec<BlockData>,
    pub cex_trades:     CexTradeMap,
    pub blocks:         usize,
}


pub struct BlockData {
    metadata:   Arc<Metadata>,
    block_tree: Arc<BlockTree<Action>>,
}
