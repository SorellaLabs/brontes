use std::{collections::HashSet, fmt, fmt::Display};

use alloy_primitives::TxHash;
use colored::Colorize;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use redefined::self_convert_redefined;
use reth_primitives::{Address, B256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, fixed_string::FixedString, Row};

use super::Node;
use crate::{
    db::{metadata::Metadata, traits::LibmdbxReader},
    normalized_actions::{Actions, NormalizedAction},
    TreeSearchArgs, TxInfo,
};

#[derive(Debug)]
pub struct NodeData<V: NormalizedAction>(Vec<Option<V>>);

impl<V: NormalizedAction> NodeData<V> {
    /// adds the node data to the storage location retuning the index
    /// that the data can be found at
    pub fn add(&mut self, data: V) -> usize {
        self.0.push(Some(data));
        self.0.len() - 1
    }

    pub fn get_ref(&self, idx: usize) -> Option<&V> {
        self.0.get(idx).and_then(|f| f.as_ref())
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut V> {
        self.0.get_mut(idx).and_then(|f| f.as_mut())
    }

    pub fn remove(&mut self, idx: usize) -> Option<V> {
        self.0[idx].take()
    }
}

#[derive(Debug)]
pub struct Root<V: NormalizedAction> {
    pub head:        Node,
    pub position:    usize,
    pub tx_hash:     B256,
    pub private:     bool,
    pub gas_details: GasDetails,
    pub data_store:  NodeData<V>,
}

impl<V: NormalizedAction> Root<V> {
    pub fn get_tx_info<DB: LibmdbxReader>(&self, block_number: u64, database: &DB) -> TxInfo {
        let to_address = self
            .data_store
            .get_ref(self.head.data)
            .unwrap()
            .clone()
            .get_action()
            .get_to_address();

        let is_verified_contract = match database.try_fetch_address_metadata(to_address) {
            Ok(metadata) => metadata.is_verified(),
            Err(_) => false,
        };
        let searcher_info = database.try_fetch_searcher_info(self.head.address).ok();

        TxInfo::new(
            block_number,
            self.position as u64,
            self.head.address,
            to_address,
            self.tx_hash,
            self.gas_details,
            self.data_store
                .get_ref(self.head.data)
                .map(|f| f.is_classified())
                .unwrap_or_default(),
            matches!(
                self.data_store.get_ref(self.head.data).unwrap().get_action(),
                Actions::Unclassified(data) if data.is_cex_dex_call()
            ),
            self.private,
            is_verified_contract,
            searcher_info,
        )
    }

    pub fn get_block_position(&self) -> usize {
        self.position
    }

    pub fn insert(&mut self, mut node: Node, data: V) {
        let idx = self.data_store.add(data);
        node.data = idx;

        self.head.insert(node)
    }

    pub fn collect_spans<F>(&self, call: &F) -> Vec<Vec<V>>
    where
        F: Fn(&Node, &NodeData<V>) -> bool,
    {
        let mut result = Vec::new();
        self.head.collect_spans(&mut result, call, &self.data_store);

        result
    }

    pub fn modify_spans<T, F>(&mut self, find: &T, modify: &F)
    where
        T: Fn(&Node, &NodeData<V>) -> bool,
        F: Fn(Vec<&mut Node>, &mut NodeData<V>),
    {
        self.head
            .modify_node_spans(find, modify, &mut self.data_store);
    }

    pub fn collect<F>(&self, call: &F) -> Vec<V>
    where
        F: Fn(&Node, &NodeData<V>) -> TreeSearchArgs,
    {
        let mut result = Vec::new();
        self.head.collect(
            &mut result,
            call,
            &|data, info| info.get_ref(data.data).unwrap().clone(),
            &self.data_store,
        );

        result.sort_by_key(|a| a.get_trace_index());

        result
    }

    pub fn modify_node_if_contains_childs<T, F>(&mut self, find: &T, modify: &F)
    where
        T: Fn(&Node, &NodeData<V>) -> TreeSearchArgs,
        F: Fn(&mut Node, &mut NodeData<V>),
    {
        self.head
            .modify_node_if_contains_childs(find, modify, &mut self.data_store);
    }

    pub fn collect_child_traces_and_classify(&mut self, heads: &[u64]) {
        heads.iter().for_each(|search_head| {
            self.head
                .get_all_children_for_complex_classification(*search_head, &mut self.data_store)
        });
    }

    pub fn remove_duplicate_data<F, C, T, R, Re>(
        &mut self,
        find: &F,
        classify: &C,
        info: &T,
        removal: &Re,
    ) where
        T: Fn(&Node, &NodeData<V>) -> R + Sync,
        C: Fn(&Vec<R>, &Node, &NodeData<V>) -> Vec<u64> + Sync,
        F: Fn(&Node, &NodeData<V>) -> TreeSearchArgs,
        Re: Fn(&Node, &NodeData<V>) -> TreeSearchArgs + Sync,
    {
        let mut find_res = Vec::new();
        self.head
            .collect(&mut find_res, find, &|data, _| data.clone(), &self.data_store);

        let indexes = find_res
            .into_par_iter()
            .flat_map(|node| {
                let mut bad_res = Vec::new();
                node.collect(&mut bad_res, removal, info, &self.data_store);
                classify(&bad_res, &node, &self.data_store)
            })
            .collect::<HashSet<_>>();

        indexes.into_iter().for_each(|index| {
            self.head
                .remove_node_and_children(index, &mut self.data_store)
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

    // Pretty print after 'spaces' spaces
    pub fn pretty_print_with_spaces(&self, f: &mut fmt::Formatter, spaces: usize) -> fmt::Result {
        let space_str = " ".repeat(spaces);
        let labels = [
            (
                "Coinbase Transfer",
                self.coinbase_transfer
                    .map(|amount| format!("{} ETH", amount))
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
    pub tx_hash:             Vec<FixedString>,
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
                    FixedString::from(format!("{:?}", tx)),
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
