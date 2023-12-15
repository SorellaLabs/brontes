use alloy_primitives::Address;
use reth_rpc_types::trace::parity::StateDiff;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Pair(pub Address, pub Address);

impl Pair {
    pub fn has_base_edge(&self, addr: Address) -> bool {
        self.0 == addr
    }

    pub fn has_quote_edge(&self, addr: Address) -> bool {
        self.1 == addr
    }
}

#[derive(Debug)]
pub struct ExtraProcessing {
    // decimals that are missing that we want to fill
    pub tokens_decimal_fill: Vec<Address>,
    // dex token prices that we need
    pub prices:              Vec<TransactionPoolSwappedTokens>,
}

#[derive(Debug)]
pub struct TransactionPoolSwappedTokens {
    pub tx_idx:     usize,
    pub pairs:      Vec<Pair>,
    pub state_diff: StateDiff,
}
