#[derive(Debug, Clone)]
pub enum Actions {
    Swap(NormalizedSwap),
    Transfer,
    Mint,
    Burn,
    Unclassified,
}

#[derive(Debug, Clone)]
pub struct NormalizedSwap {
    fn_name: String,
}

pub trait NormalizedAction: Clone {
    fn get_action(&self) -> &Actions;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        &self
    }
}
