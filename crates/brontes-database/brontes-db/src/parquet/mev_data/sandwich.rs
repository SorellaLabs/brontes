use std::sync::Arc;

use arrow::{
    array::Array,
    datatypes::{Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::Sandwich;
use itertools::Itertools;

use crate::parquet::{
    normalized_actions::{
        gas_details::{get_gas_details_array, get_gas_details_list_array},
        swaps::get_normalized_swap_list_array,
    },
    utils::{get_list_string_array_from_owned, get_string_array_from_owned},
};

pub fn sandwich_to_record_batch(sandwiches: Vec<Sandwich>) -> Result<RecordBatch, ArrowError> {
    let frontrun_tx_hash_array = get_list_string_array_from_owned(
        sandwiches
            .iter()
            .map(|s| {
                s.frontrun_tx_hash
                    .iter()
                    .map(|hash| hash.to_string())
                    .collect_vec()
            })
            .collect_vec(),
    );

    let frontrun_swaps_array = get_normalized_swap_list_array(
        sandwiches
            .iter()
            .map(|s| s.frontrun_swaps.iter().flatten().collect_vec())
            .collect_vec(),
    );

    let frontrun_gas_details_array = get_gas_details_list_array(
        sandwiches
            .iter()
            .map(|s| &s.frontrun_gas_details)
            .collect_vec(),
    );

    let victim_swaps_tx_hashes_array = get_list_string_array_from_owned(
        sandwiches
            .iter()
            .map(|s| {
                s.victim_swaps_tx_hashes
                    .iter()
                    .flatten()
                    .map(|hash| hash.to_string())
                    .collect_vec()
            })
            .collect_vec(),
    );
    let victim_swaps_array = get_normalized_swap_list_array(
        sandwiches
            .iter()
            .map(|s| s.victim_swaps.iter().flatten().collect_vec())
            .collect_vec(),
    );

    let victim_swaps_gas_details_array = get_gas_details_list_array(
        sandwiches
            .iter()
            .map(|s| &s.victim_swaps_gas_details)
            .collect_vec(),
    );

    let backrun_tx_hash_array = get_string_array_from_owned(
        sandwiches
            .iter()
            .map(|s| Some(s.backrun_tx_hash.to_string()))
            .collect_vec(),
    );

    let backrun_swaps_array = get_normalized_swap_list_array(
        sandwiches
            .iter()
            .map(|s| s.backrun_swaps.iter().collect_vec())
            .collect_vec(),
    );

    let backrun_gas_details_array =
        get_gas_details_array(sandwiches.iter().map(|s| s.backrun_gas_details).collect());

    let schema = Schema::new(vec![
        Field::new("frontrun_tx_hash", frontrun_tx_hash_array.data_type().clone(), false),
        Field::new("frontrun_swaps", frontrun_swaps_array.data_type().clone(), false),
        Field::new("frontrun_gas_details", frontrun_gas_details_array.data_type().clone(), false),
        Field::new(
            "victim_swaps_tx_hashes",
            victim_swaps_tx_hashes_array.data_type().clone(),
            false,
        ),
        Field::new("victim_swaps", victim_swaps_array.data_type().clone(), false),
        Field::new(
            "victim_swaps_gas_details",
            victim_swaps_gas_details_array.data_type().clone(),
            false,
        ),
        Field::new("backrun_tx_hash", backrun_tx_hash_array.data_type().clone(), false),
        Field::new("backrun_swaps", backrun_swaps_array.data_type().clone(), false),
        Field::new("backrun_gas_details", backrun_gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(frontrun_tx_hash_array),
            Arc::new(frontrun_swaps_array),
            Arc::new(frontrun_gas_details_array),
            Arc::new(victim_swaps_tx_hashes_array),
            Arc::new(victim_swaps_array),
            Arc::new(victim_swaps_gas_details_array),
            Arc::new(backrun_tx_hash_array),
            Arc::new(backrun_swaps_array),
            Arc::new(backrun_gas_details_array),
        ],
    )
}
