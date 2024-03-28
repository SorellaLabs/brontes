use std::sync::Arc;

use arrow::{
    array::Array,
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::AtomicArb;
use itertools::Itertools;

use crate::parquet::{
    normalized_actions::{
        gas_details::get_gas_details_array, swaps::get_normalized_swap_list_array,
    },
    utils::get_string_array_from_owned,
};

pub fn atomic_arb_to_record_batch(atomic_arbs: Vec<AtomicArb>) -> Result<RecordBatch, ArrowError> {
    let tx_hash_array = get_string_array_from_owned(
        atomic_arbs
            .iter()
            .map(|arb| Some(arb.tx_hash.to_string()))
            .collect_vec(),
    );

    let swaps_array = get_normalized_swap_list_array(
        atomic_arbs
            .iter()
            .map(|arb| arb.swaps.iter().collect_vec())
            .collect_vec(),
    );

    let gas_details_array =
        get_gas_details_array(atomic_arbs.iter().map(|arb| arb.gas_details).collect());

    let arb_type_array = get_string_array_from_owned(
        atomic_arbs
            .iter()
            .map(|arb| Some(arb.arb_type.to_string()))
            .collect_vec(),
    );

    let schema = Schema::new(vec![
        Field::new("tx_hash", DataType::Utf8, false),
        Field::new("swaps", swaps_array.data_type().clone(), false),
        Field::new("gas_details", gas_details_array.data_type().clone(), false),
        Field::new("arb_type", DataType::Utf8, false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(tx_hash_array),
            Arc::new(swaps_array),
            Arc::new(gas_details_array),
            Arc::new(arb_type_array),
        ],
    )
}
