use alloy_primitives::{Address, TxHash};

use crate::GasDetails;

#[derive(Debug, Clone, Copy)]
pub struct TxInfo {
    pub block_number:    u64,
    pub tx_index:        u64,
    pub eoa:             Address,
    pub mev_contract:    Address,
    pub tx_hash:         TxHash,
    pub gas_details:     GasDetails,
    pub is_classifed:    bool,
    pub is_cex_dex_call: bool,
}

impl TxInfo {
    pub fn new(
        block_number: u64,
        tx_index: u64,
        eoa: Address,
        mev_contract: Address,
        tx_hash: TxHash,
        gas_details: GasDetails,
        is_classifed: bool,
        is_cex_dex_call: bool,
    ) -> Self {
        Self {
            tx_index,
            block_number,
            mev_contract,
            eoa,
            tx_hash,
            is_classifed,
            gas_details,
            is_cex_dex_call,
        }
    }

    pub fn split_to_storage_info(self) -> (TxHash, GasDetails) {
        (self.tx_hash, self.gas_details)
    }
}
