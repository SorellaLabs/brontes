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

    pub fn map_key(addr1: Address, addr2: Address) -> Self {
        if addr1 <= addr2 {
            Pair(addr1, addr2)
        } else {
            Pair(addr2, addr1)
        }
    }

    // Returns an ordered version of the pair
    pub fn ordered(&self) -> Self {
        if self.0 <= self.1 {
            Pair(self.0.clone(), self.1.clone())
        } else {
            Pair(self.1.clone(), self.0.clone())
        }
    }
}

impl Pair {}

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
