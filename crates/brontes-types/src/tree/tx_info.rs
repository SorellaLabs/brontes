use alloy_primitives::{Address, TxHash};

use crate::{db::searcher::SearcherInfo, mev::MevType, GasDetails};

#[derive(Debug, Clone)]
pub struct TxInfo {
    pub block_number:           u64,
    pub tx_index:               u64,
    pub eoa:                    Address,
    // is none if the contract is classified, or emits logs
    // or is verified
    pub mev_contract:           Option<Address>,
    pub tx_hash:                TxHash,
    pub gas_details:            GasDetails,
    pub is_classified:          bool,
    pub is_cex_dex_call:        bool,
    pub is_private:             bool,
    pub is_verified_contract:   bool,
    pub searcher_eoa_info:      Option<SearcherInfo>,
    pub searcher_contract_info: Option<SearcherInfo>,
}

impl TxInfo {
    pub fn new(
        block_number: u64,
        tx_index: u64,
        eoa: Address,
        mev_contract: Option<Address>,
        tx_hash: TxHash,
        gas_details: GasDetails,
        is_classified: bool,
        is_cex_dex_call: bool,
        is_private: bool,
        is_verified_contract: bool,
        searcher_eoa_info: Option<SearcherInfo>,
        searcher_contract_info: Option<SearcherInfo>,
    ) -> Self {
        Self {
            tx_index,
            block_number,
            mev_contract,
            eoa,
            tx_hash,
            gas_details,
            is_classified,
            is_cex_dex_call,
            is_private,
            is_verified_contract,
            searcher_eoa_info,
            searcher_contract_info,
        }
    }

    pub fn split_to_storage_info(self) -> (TxHash, GasDetails) {
        (self.tx_hash, self.gas_details)
    }

    pub fn get_searcher_eao_info(&self) -> Option<&SearcherInfo> {
        self.searcher_eoa_info.as_ref()
    }

    pub fn get_searcher_contract_info(&self) -> Option<&SearcherInfo> {
        self.searcher_contract_info.as_ref()
    }

    pub fn is_searcher_of_type(&self, mev_type: MevType) -> bool {
        let eoa_contains_type = self
            .searcher_eoa_info
            .as_ref()
            .map_or(false, |info| info.contains_searcher_type(mev_type));
        let contract_contains_type = self
            .searcher_contract_info
            .as_ref()
            .map_or(false, |info| info.contains_searcher_type(mev_type));
        eoa_contains_type || contract_contains_type
    }

    pub fn is_private(&self) -> bool {
        self.is_private
    }

    pub fn is_verified_contract(&self) -> bool {
        self.is_verified_contract
    }

    pub fn is_classified(&self) -> bool {
        self.is_classified
    }

    pub fn is_cex_dex_call(&self) -> bool {
        self.is_cex_dex_call
    }
}
