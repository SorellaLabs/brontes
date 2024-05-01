use std::sync::Arc;

use arrow::{
    array::Array,
    datatypes::{Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::JitLiquiditySandwich;
use itertools::Itertools;

use crate::parquet::{
    normalized_actions::{
        burns::get_normalized_burn_list_array,
        gas_details::{get_gas_details_array, get_gas_details_list_array},
        mints::get_normalized_mint_list_array,
        swaps::get_normalized_swap_list_array,
    },
    utils::{get_list_string_array_from_owned, get_string_array_from_owned},
};

pub fn jit_sandwich_to_record_batch(
    jit_liquidity_sandwiches: Vec<JitLiquiditySandwich>,
) -> Result<RecordBatch, ArrowError> {
    let frontrun_tx_hash_array = get_list_string_array_from_owned(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| {
                jls.frontrun_tx_hash
                    .iter()
                    .map(|hash| hash.to_string())
                    .collect()
            })
            .collect(),
    );

    let frontrun_swaps_array = get_normalized_swap_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| jls.frontrun_swaps.iter().flatten().collect())
            .collect(),
    );

    let frontrun_mints_array = get_normalized_mint_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| {
                jls.frontrun_mints
                    .iter()
                    .filter_map(|m| m.as_ref())
                    .flatten()
                    .collect_vec()
            })
            .collect_vec(),
    );

    let frontrun_gas_details_array = get_gas_details_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| &jls.frontrun_gas_details)
            .collect(),
    );

    let victim_swaps_tx_hashes_array = get_list_string_array_from_owned(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| {
                jls.victim_swaps_tx_hashes
                    .iter()
                    .flatten()
                    .map(|hash| hash.to_string())
                    .collect()
            })
            .collect(),
    );

    let victim_swaps_array = get_normalized_swap_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| jls.victim_swaps.iter().flatten().collect_vec())
            .collect_vec(),
    );

    let victim_swaps_gas_details_array = get_gas_details_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| &jls.victim_swaps_gas_details)
            .collect(),
    );

    let backrun_tx_hash_array = get_string_array_from_owned(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| Some(jls.backrun_tx_hash.to_string()))
            .collect(),
    );

    let backrun_swaps_array = get_normalized_swap_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| jls.backrun_swaps.iter().collect())
            .collect(),
    );

    let backrun_burns_array = get_normalized_burn_list_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| &jls.backrun_burns)
            .collect_vec(),
    );

    let backrun_gas_details_array = get_gas_details_array(
        jit_liquidity_sandwiches
            .iter()
            .map(|jls| jls.backrun_gas_details)
            .collect(),
    );

    let schema = Schema::new(vec![
        Field::new("frontrun_tx_hashes", frontrun_tx_hash_array.data_type().clone(), false),
        Field::new("frontrun_swaps", frontrun_swaps_array.data_type().clone(), false),
        Field::new("frontrun_mints", frontrun_mints_array.data_type().clone(), false),
        Field::new("frontrun_gas_details", frontrun_gas_details_array.data_type().clone(), false),
        Field::new("victim_tx_hashes", victim_swaps_tx_hashes_array.data_type().clone(), false),
        Field::new("victim_swaps", victim_swaps_array.data_type().clone(), false),
        Field::new("victim_gas_details", victim_swaps_gas_details_array.data_type().clone(), false),
        Field::new("backrun_tx_hash", backrun_tx_hash_array.data_type().clone(), false),
        Field::new("backrun_swaps", backrun_swaps_array.data_type().clone(), false),
        Field::new("backrun_burns", backrun_burns_array.data_type().clone(), false),
        Field::new("backrun_gas_details", backrun_gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(frontrun_tx_hash_array),
            Arc::new(frontrun_swaps_array),
            Arc::new(frontrun_mints_array),
            Arc::new(frontrun_gas_details_array),
            Arc::new(victim_swaps_tx_hashes_array),
            Arc::new(victim_swaps_array),
            Arc::new(victim_swaps_gas_details_array),
            Arc::new(backrun_tx_hash_array),
            Arc::new(backrun_swaps_array),
            Arc::new(backrun_burns_array),
            Arc::new(backrun_gas_details_array),
        ],
    )
}
