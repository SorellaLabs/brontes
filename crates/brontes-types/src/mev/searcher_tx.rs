use std::fmt::Debug;

use ::serde::ser::Serializer;
#[allow(unused)]
use clickhouse::row::*;
use clickhouse::Row;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::redefined_types::primitives::*,
    mev::{Mev, MevType},
    normalized_actions::*,
};
#[allow(unused_imports)]
use crate::{display::utils::display_sandwich, normalized_actions::NormalizedTransfer, GasDetails};

#[serde_as]
#[derive(Debug, Row, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherTx {
    pub tx_hash:            B256,
    pub searcher_transfers: Vec<NormalizedTransfer>,
    #[redefined(same_fields)]
    pub gas_details:        GasDetails,
}

impl Mev for SearcherTx {
    fn mev_type(&self) -> MevType {
        MevType::Unknown
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn total_gas_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.gas_details.priority_fee_paid(base_fee)
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

impl Serialize for SearcherTx {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!();
    }
}
