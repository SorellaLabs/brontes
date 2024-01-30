use std::{collections::HashSet, fmt, fmt::Display};

use alloy_primitives::TxHash;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_primitives::{Address, B256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, fixed_string::FixedString, Row};

use super::Node;
use crate::{normalized_actions::NormalizedAction, tree::MetadataNoDex};

#[derive(Debug, Serialize, Deserialize)]
pub struct Root<V: NormalizedAction> {
    pub head:        Node<V>,
    pub position:    usize,
    pub tx_hash:     B256,
    pub private:     bool,
    pub gas_details: GasDetails,
}

impl<V: NormalizedAction> Root<V> {
    pub fn get_block_position(&self) -> usize {
        self.position
    }

    pub fn insert(&mut self, node: Node<V>) {
        self.head.insert(node)
    }

    pub fn collect_spans<F>(&self, call: &F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        let mut result = Vec::new();
        self.head.collect_spans(&mut result, call);

        result
    }

    pub fn collect<F>(&self, call: &F) -> Vec<V>
    where
        F: Fn(&Node<V>) -> (bool, bool),
    {
        let mut result = Vec::new();
        self.head
            .collect(&mut result, call, &|data| data.data.clone());

        result
    }

    pub fn modify_node_if_contains_childs<T, F>(&mut self, find: &T, modify: &F)
    where
        T: Fn(&Node<V>) -> (bool, bool),
        F: Fn(&mut Node<V>),
    {
        self.head.modify_node_if_contains_childs(find, modify);
    }

    pub fn collect_child_traces_and_classify(&mut self, heads: &Vec<u64>) {
        heads.into_iter().for_each(|search_head| {
            self.head
                .get_all_children_for_complex_classification(*search_head)
        });
    }

    pub fn remove_duplicate_data<F, C, T, R, Re>(
        &mut self,
        find: &F,
        classify: &C,
        info: &T,
        removal: &Re,
    ) where
        T: Fn(&Node<V>) -> R + Sync,
        C: Fn(&Vec<R>, &Node<V>) -> Vec<u64> + Sync,
        F: Fn(&Node<V>) -> (bool, bool),
        Re: Fn(&Node<V>) -> (bool, bool) + Sync,
    {
        let mut find_res = Vec::new();
        self.head.collect(&mut find_res, find, &|data| data.clone());

        let indexes = find_res
            .into_par_iter()
            .flat_map(|node| {
                let mut bad_res = Vec::new();
                node.collect(&mut bad_res, removal, info);
                classify(&bad_res, &node)
            })
            .collect::<HashSet<_>>();

        indexes
            .into_iter()
            .for_each(|index| self.head.remove_node_and_children(index));
    }

    pub fn dyn_classify<T, F>(&mut self, find: &T, call: &F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, &Node<V>) -> (bool, bool),
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        // bool is used for recursion
        let mut results = Vec::new();
        let _ = self.head.dyn_classify(find, call, &mut results);

        results
    }

    pub fn finalize(&mut self) {
        self.head.finalize();
    }

    pub fn is_private(&self) -> bool {
        self.private
    }

    pub fn label_private_tx(&mut self, metadata: &MetadataNoDex) {
        if metadata.private_flow.contains(&self.tx_hash) {
            self.private = true;
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
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
