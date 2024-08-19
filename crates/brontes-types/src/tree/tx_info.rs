use alloy_primitives::{Address, TxHash};

use crate::{
    db::{address_metadata::ContractType, searcher::SearcherInfo},
    mev::MevType,
    normalized_actions::NormalizedEthTransfer,
    FastHashSet, GasDetails,
};

#[derive(Debug, Clone)]
pub struct TxInfo {
    pub block_number: u64,
    pub tx_index:     u64,
    pub eoa:          Address,

    // is none if the contract is classified, or emits logs
    // or is verified
    pub mev_contract:           Option<Address>,
    pub contract_type:          Option<ContractType>,
    pub tx_hash:                TxHash,
    pub gas_details:            GasDetails,
    pub is_classified:          bool,
    pub is_cex_dex_call:        bool,
    pub is_private:             bool,
    pub is_verified_contract:   bool,
    pub searcher_eoa_info:      Option<SearcherInfo>,
    pub searcher_contract_info: Option<SearcherInfo>,
    pub total_eth_value:        Vec<NormalizedEthTransfer>,
}

impl TxInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_number: u64,
        tx_index: u64,
        eoa: Address,
        mev_contract: Option<Address>,
        contract_type: Option<ContractType>,
        tx_hash: TxHash,
        gas_details: GasDetails,
        is_classified: bool,
        is_cex_dex_call: bool,
        is_private: bool,
        is_verified_contract: bool,
        searcher_eoa_info: Option<SearcherInfo>,
        searcher_contract_info: Option<SearcherInfo>,
        total_eth_value: Vec<NormalizedEthTransfer>,
    ) -> Self {
        Self {
            total_eth_value,
            tx_index,
            block_number,
            mev_contract,
            contract_type,
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

    pub fn get_total_eth_value(&self) -> &[NormalizedEthTransfer] {
        &self.total_eth_value
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

    pub fn collect_address_set_for_accounting(&self) -> FastHashSet<Address> {
        let mut mev_addresses: FastHashSet<Address> = vec![self.eoa]
            .into_iter()
            .chain(self.mev_contract)
            .collect();

        self.get_sibling_searchers(&mut mev_addresses);
        mev_addresses
    }

    pub fn get_sibling_searchers(&self, searchers: &mut FastHashSet<Address>) {
        if let Some(ref searcher_info) = self.searcher_eoa_info {
            for address in searcher_info.get_sibling_searchers() {
                searchers.insert(*address);
            }
        }

        if let Some(ref searcher_info) = self.searcher_contract_info {
            for address in searcher_info.get_sibling_searchers() {
                searchers.insert(*address);
            }
        }
    }

    pub fn is_searcher_of_type(&self, mev_type: MevType) -> bool {
        self.searcher_eoa_info
            .as_ref()
            .map_or(false, |info| info.is_searcher_of_type(mev_type))
            || self
                .searcher_contract_info
                .as_ref()
                .map_or(false, |info| info.is_searcher_of_type(mev_type))
    }

    pub fn is_searcher_of_type_with_count_threshold(
        &self,
        mev_type: MevType,
        threshold: u64,
    ) -> bool {
        self.searcher_eoa_info
            .as_ref()
            .map_or(false, |info| info.is_searcher_of_type_with_threshold(mev_type, threshold))
            || self
                .searcher_contract_info
                .as_ref()
                .map_or(false, |info| info.is_searcher_of_type_with_threshold(mev_type, threshold))
    }

    pub fn infer_mev_bot_type(&self) -> Option<MevType> {
        self.searcher_contract_info
            .as_ref()
            .and_then(|info| info.infer_mev_bot_type())
            .or(self
                .searcher_eoa_info
                .as_ref()
                .and_then(|info| info.infer_mev_bot_type()))
    }

    pub fn is_labelled_searcher_of_type(&self, mev_type: MevType) -> bool {
        self.searcher_eoa_info
            .as_ref()
            .map_or(false, |info| info.is_labelled_searcher_of_type(mev_type))
            || self
                .searcher_contract_info
                .as_ref()
                .map_or(false, |info| info.is_labelled_searcher_of_type(mev_type))
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

pub fn collect_address_set_for_accounting(tx_infos: &[TxInfo]) -> FastHashSet<Address> {
    let mut mev_addresses: FastHashSet<Address> = tx_infos
        .iter()
        .flat_map(|tx_info| std::iter::once(tx_info.eoa).chain(tx_info.mev_contract))
        .collect();

    for tx_info in tx_infos {
        tx_info.get_sibling_searchers(&mut mev_addresses);
    }

    mev_addresses
}
