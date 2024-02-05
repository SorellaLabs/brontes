#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreeSearchArgs {
    pub collect_current_node:  bool,
    pub child_node_to_collect: bool,
}
