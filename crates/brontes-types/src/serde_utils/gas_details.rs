use alloy_primitives::TxHash;
use itertools::Itertools;
use sorella_db_databases::clickhouse::fixed_string::FixedString;

use crate::tree::GasDetails;

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
