use std::{fmt, fmt::Display};

use alloy_primitives::{Address, TxHash, B256};
use clickhouse::Row;
use colored::Colorize;
use itertools::Itertools;
use redefined::self_convert_redefined;
use serde::{Deserialize, Serialize};

use super::Node;
use crate::{
    db::{
        address_metadata::AddressMetadata, metadata::Metadata, searcher::SearcherInfo,
        traits::LibmdbxReader,
    },
    normalized_actions::{
        Action, MultiCallFrameClassification, NormalizedAction, NormalizedEthTransfer,
    },
    tree::types::NodeWithDataRef,
    FastHashMap, FastHashSet, TreeSearchBuilder, TxInfo,
};

#[derive(Debug, Clone)]
pub struct NodeData<V: NormalizedAction>(pub Vec<Option<Vec<V>>>);

impl<V: NormalizedAction> NodeData<V> {
    /// adds the node data to the storage location retuning the index
    /// that the data can be found at
    pub fn add(&mut self, data: Vec<V>) -> usize {
        self.0.push(Some(data));
        self.0.len() - 1
    }

    pub fn get_ref(&self, idx: usize) -> Option<&Vec<V>> {
        self.0.get(idx).and_then(|f| f.as_ref())
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Vec<V>> {
        self.0.get_mut(idx).and_then(|f| f.as_mut())
    }

    pub fn remove(&mut self, idx: usize) -> Option<Vec<V>> {
        self.0[idx].take()
    }

    pub fn replace(&mut self, idx: usize, value: Vec<V>) {
        self.0[idx] = Some(value);
    }
}

#[derive(Debug, Clone)]
pub struct Root<V: NormalizedAction> {
    pub head: Node,
    pub position: usize,
    pub tx_hash: B256,
    pub private: bool,
    pub gas_details: GasDetails,
    /// all msg.value transfers that aren't classified as
    /// eth transfers
    pub total_msg_value_transfers: Vec<NormalizedEthTransfer>,
    pub data_store: NodeData<V>,
}

impl<V: NormalizedAction> Root<V> {
    //TODO: Add field for reinit bool flag
    //TODO: Once metadata table is updated
    //TODO: Filter out know entities from address metadata enum variant or contract
    // info struct

    pub fn get_tx_info_batch(
        &self,
        block_number: u64,
        eoa: &FastHashMap<Address, SearcherInfo>,
        contract: &FastHashMap<Address, SearcherInfo>,
        address_meta: &FastHashMap<Address, AddressMetadata>,
    ) -> eyre::Result<TxInfo> {
        self.tx_info_internal(
            block_number,
            |eoa_addr| Ok(eoa.get(&eoa_addr).cloned()),
            |contract_addr| Ok(contract.get(&contract_addr).cloned()),
            |address_metadata| Ok(address_meta.get(&address_metadata).cloned()),
        )
    }

    pub fn tx_must_contain_action(&self, f: impl Fn(&V) -> bool) -> bool {
        self.data_store.0.iter().flatten().flatten().any(f)
    }

    pub fn get_tx_info<DB: LibmdbxReader>(
        &self,
        block_number: u64,
        database: &DB,
    ) -> eyre::Result<TxInfo> {
        self.tx_info_internal(
            block_number,
            |eoa_addr| database.try_fetch_searcher_eoa_info(eoa_addr),
            |contract_addr| database.try_fetch_searcher_contract_info(contract_addr),
            |address_meta| database.try_fetch_address_metadata(address_meta),
        )
    }

    fn tx_info_internal(
        &self,
        block_number: u64,
        eoa: impl Fn(Address) -> eyre::Result<Option<SearcherInfo>>,
        contract: impl Fn(Address) -> eyre::Result<Option<SearcherInfo>>,
        address: impl Fn(Address) -> eyre::Result<Option<AddressMetadata>>,
    ) -> eyre::Result<TxInfo> {
        let to_address = self
            .data_store
            .get_ref(self.head.data)
            .unwrap()
            .clone()
            .first()
            .unwrap()
            .get_action()
            .get_to_address();

        let address_meta =
            address(to_address).map_err(|_| eyre::eyre!("Failed to fetch address metadata"))?;

        let (is_verified_contract, contract_type) = match address_meta {
            Some(meta) => {
                let verified = meta.is_verified();
                let contract_type = meta.get_contract_type();

                (verified, Some(contract_type))
            }
            None => (false, None),
        };

        let is_classified = self
            .data_store
            .get_ref(self.head.data)
            .map(|f| f.iter().any(|f| f.is_classified()))
            .unwrap_or_default();

        let emits_logs = self
            .data_store
            .get_ref(self.head.data)
            .unwrap()
            .iter()
            .any(|a| a.get_action().emitted_logs());

        // TODO: get rid of this once searcher db is working & tested

        let is_cex_dex_call = self
            .data_store
            .get_ref(self.head.data)
            .unwrap()
            .iter()
            .any(|a| {
                matches!(a.get_action(),
                Action::Unclassified(data) if data.is_cex_dex_call()
                )
            });

        let searcher_eoa_info = eoa(self.head.address)?;
        let searcher_contract_info = contract(self.get_to_address())?;

        // If the to address is a verified contract, or emits logs, or is classified
        // then shouldn't pass it as mev_contract to avoid the misclassification of
        // protocol addresses as mev contracts
        if is_verified_contract
            || is_classified
            || emits_logs && searcher_contract_info.is_none()
            || contract_type
                .as_ref()
                .map_or(false, |ct| !ct.could_be_mev_contract())
        {
            return Ok(TxInfo::new(
                block_number,
                self.position as u64,
                self.head.address,
                None,
                contract_type,
                self.tx_hash,
                self.gas_details,
                is_classified,
                is_cex_dex_call,
                self.private,
                is_verified_contract,
                searcher_eoa_info,
                None,
                self.total_msg_value_transfers.clone(),
            ))
        }

        Ok(TxInfo::new(
            block_number,
            self.position as u64,
            self.head.address,
            Some(to_address),
            contract_type,
            self.tx_hash,
            self.gas_details,
            is_classified,
            is_cex_dex_call,
            self.private,
            is_verified_contract,
            searcher_eoa_info,
            searcher_contract_info,
            self.total_msg_value_transfers.clone(),
        ))
    }

    pub fn get_from_address(&self) -> Address {
        self.head.address
    }

    pub fn try_get_to_address(&self) -> Option<Address> {
        Some(
            self.data_store
                .get_ref(0)?
                .first()?
                .get_action()
                .get_to_address(),
        )
    }

    pub fn get_to_address(&self) -> Address {
        self.data_store
            .get_ref(0)
            .unwrap()
            .first()
            .unwrap()
            .get_action()
            .get_to_address()
    }

    pub fn get_root_action(&self) -> &V {
        self.data_store.get_ref(0).unwrap().first().unwrap()
    }

    pub fn get_block_position(&self) -> usize {
        self.position
    }

    pub fn insert(&mut self, node: Node, data: Vec<V>) {
        self.head.insert(node, data, &mut self.data_store);
    }

    pub fn collect_spans(&self, call: &TreeSearchBuilder<V>) -> Vec<Vec<V>> {
        let mut result = Vec::new();
        self.head.collect_spans(&mut result, call, &self.data_store);

        result
    }

    pub fn modify_spans<F>(&mut self, find: &TreeSearchBuilder<V>, modify: &F)
    where
        F: Fn(Vec<&mut Node>, &mut NodeData<V>),
    {
        self.head
            .modify_node_spans(find, modify, &mut self.data_store);
    }

    pub fn collect(&self, call: &TreeSearchBuilder<V>) -> Vec<V> {
        let mut result = Vec::new();
        self.head
            .collect(&mut result, call, &|data| data.data.clone(), &self.data_store);

        result.sort_by_key(|a| a.get_trace_index());

        result
    }

    pub fn modify_node_if_contains_childs<F>(&mut self, find: &TreeSearchBuilder<V>, modify: &F)
    where
        F: Fn(&mut Node, &mut NodeData<V>),
    {
        self.head
            .modify_node_if_contains_childs(find, modify, &mut self.data_store);
    }

    pub fn collect_child_traces_and_classify(&mut self, heads: &[MultiCallFrameClassification<V>]) {
        heads.iter().for_each(|search_head| {
            self.head
                .get_all_children_for_complex_classification(search_head, &mut self.data_store)
        });
    }

    pub fn finalize(&mut self) {
        self.head.finalize();
    }

    pub fn is_private(&self) -> bool {
        self.private
    }

    pub fn label_private_tx(&mut self, metadata: &Metadata) {
        if metadata.private_flow.contains(&self.tx_hash) {
            self.private = true;
        }
    }

    pub fn remove_duplicate_data<C, T, R>(
        &mut self,
        find: &TreeSearchBuilder<V>,
        classify: &C,
        info: &T,
        removal: &TreeSearchBuilder<V>,
    ) where
        T: Fn(NodeWithDataRef<'_, V>) -> R + Sync,
        C: Fn(&Vec<R>, &Node, &NodeData<V>) -> Vec<u64> + Sync,
    {
        let mut find_res = Vec::new();
        self.head
            .collect(&mut find_res, find, &|data| data.node.clone(), &self.data_store);

        let mut bad_res: Vec<R> = Vec::new();
        self.head
            .collect(&mut bad_res, removal, info, &self.data_store);

        let indexes = find_res
            .into_iter()
            .flat_map(|node| classify(&bad_res, &node, &self.data_store))
            .collect::<FastHashSet<_>>();

        indexes.into_iter().for_each(|index| {
            self.head
                .remove_node_and_children(index, &mut self.data_store)
        });
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Serialize,
    Deserialize,
    Row,
    Default,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
pub struct GasDetails {
    pub coinbase_transfer:   Option<u128>,
    pub priority_fee:        u128,
    pub gas_used:            u128,
    pub effective_gas_price: u128,
}
//TODO: Fix this
impl Display for GasDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GasDetails {{ coinbase_transfer: {:?}, priority_fee: {}, gas_used: {}, \
             effective_gas_price: {} }}",
            self.coinbase_transfer, self.priority_fee, self.gas_used, self.effective_gas_price
        )
    }
}

self_convert_redefined!(GasDetails);

impl GasDetails {
    pub fn gas_paid(&self) -> u128 {
        let mut gas = self.gas_used * self.effective_gas_price;

        if let Some(coinbase) = self.coinbase_transfer {
            gas += coinbase
        }

        gas
    }

    pub fn priority_fee(&self, base_fee: u128) -> u128 {
        self.effective_gas_price - base_fee
    }

    pub fn priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.priority_fee(base_fee) * self.gas_used
    }

    pub fn coinbase_transfer(&self) -> u128 {
        self.coinbase_transfer.unwrap_or_default()
    }

    pub fn merge(&mut self, other: &GasDetails) {
        self.coinbase_transfer = Some(
            self.coinbase_transfer.unwrap_or_default()
                + other.coinbase_transfer.unwrap_or_default(),
        )
        .filter(|&res| res != 0);

        self.priority_fee += other.priority_fee;
        self.gas_used += other.gas_used;
        self.effective_gas_price += other.effective_gas_price;
    }

    // Pretty print after 'spaces' spaces
    pub fn pretty_print_with_spaces(&self, f: &mut fmt::Formatter, spaces: usize) -> fmt::Result {
        let space_str = " ".repeat(spaces);
        let labels = [
            (
                "Coinbase Transfer",
                self.coinbase_transfer
                    .map(|amount| format!("{:.18} ETH", amount as f64 / 1e18))
                    .unwrap_or_else(|| "None".to_string()),
            ),
            ("Priority Fee", format!("{} Wei", self.priority_fee)),
            ("Gas Used", self.gas_used.to_string()),
            ("Effective Gas Price", format!("{} Wei", self.effective_gas_price)),
            ("Total Gas Paid in ETH", format!("{:.7} ETH", self.gas_paid() as f64 / 1e18)),
        ];

        let max_label_length = labels
            .iter()
            .map(|(label, _)| label.len())
            .max()
            .unwrap_or(0);

        for (label, value) in &labels {
            writeln!(
                f,
                "{}",
                self.format_line_with_spaces(label, value, max_label_length, &space_str)
            )?;
        }

        Ok(())
    }

    fn format_line_with_spaces(
        &self,
        label: &str,
        value: &str,
        max_label_length: usize,
        leading_spaces: &str,
    ) -> String {
        let padded_label = format!("{:<width$} :", label, width = max_label_length);
        let formatted_value = format!("    {}", value).bright_yellow();
        format!("{}{}{}", leading_spaces, padded_label, formatted_value)
    }
}

pub struct ClickhouseVecGasDetails {
    pub tx_hash:             Vec<String>,
    pub coinbase_transfer:   Vec<Option<u128>>,
    pub priority_fee:        Vec<u128>,
    pub gas_used:            Vec<u128>,
    pub effective_gas_price: Vec<u128>,
}

impl From<(Vec<TxHash>, Vec<GasDetails>)> for ClickhouseVecGasDetails {
    fn from(value: (Vec<TxHash>, Vec<GasDetails>)) -> Self {
        let vec_vals = value
            .0
            .into_iter()
            .zip(value.1)
            .map(|(tx, gas)| {
                (
                    format!("{:?}", tx),
                    gas.coinbase_transfer,
                    gas.priority_fee,
                    gas.gas_used,
                    gas.effective_gas_price,
                )
            })
            .collect::<Vec<_>>();

        ClickhouseVecGasDetails {
            tx_hash:             vec_vals.iter().map(|val| val.0.to_owned()).collect_vec(),
            coinbase_transfer:   vec_vals.iter().map(|val| val.1.to_owned()).collect_vec(),
            priority_fee:        vec_vals.iter().map(|val| val.2.to_owned()).collect_vec(),
            gas_used:            vec_vals.iter().map(|val| val.3.to_owned()).collect_vec(),
            effective_gas_price: vec_vals.iter().map(|val| val.4.to_owned()).collect_vec(),
        }
    }
}

/// i.e. Sandwich: From <victim_tx_hashes, victim_swaps)
impl From<(Vec<Vec<TxHash>>, Vec<GasDetails>)> for ClickhouseVecGasDetails {
    fn from(value: (Vec<Vec<TxHash>>, Vec<GasDetails>)) -> Self {
        let tx_hashes = value.0.into_iter().flatten().collect_vec();
        let gas_details = value.1;

        (tx_hashes, gas_details).into()
    }
}

pub enum FalsePositiveEntity {
    MaestroBots,
}

/*
#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::{get_db_handle, ClassifierTestUtils};
    use brontes_types::{normalized_actions::Action, tree::BlockTree};

    use super::*;

    #[brontes_macros::test]
    async fn test_tx_info_filters() {
        let handle = tokio::runtime::Handle::current();
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("d6aa973068528615f4bba657b9b3366166c1ea0f56ac1313afe7abd97668ae4f").into();

        let tree: Arc<BlockTree<Action>> =
            Arc::new(classifier_utils.build_tree_tx(tx).await.unwrap());

        let info = tree
            .get_tx_info(tx, classifier_utils.)
            .unwrap();

        assert_eq!(info.mev_contract, None)
    }
}*/
