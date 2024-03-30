use std::sync::Arc;

use arrow::{
    array::Array,
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::Liquidation;
use itertools::Itertools;

use crate::parquet::{
    normalized_actions::{
        gas_details::get_gas_details_array, liquidations::get_normalized_liquidation_list_array,
        swaps::get_normalized_swap_list_array,
    },
    utils::get_string_array_from_owned,
};

pub fn liquidation_to_record_batch(
    liquidations: Vec<Liquidation>,
) -> Result<RecordBatch, ArrowError> {
    let liquidation_tx_hash_array = get_string_array_from_owned(
        liquidations
            .iter()
            .map(|liq| Some(liq.liquidation_tx_hash.to_string()))
            .collect(),
    );

    let trigger_array = get_string_array_from_owned(
        liquidations
            .iter()
            .map(|liq| Some(liq.trigger.to_string()))
            .collect(),
    );

    let liquidation_swaps_array = get_normalized_swap_list_array(
        liquidations
            .iter()
            .map(|liq| liq.liquidation_swaps.iter().collect_vec())
            .collect_vec(),
    );

    let liquidations_array = get_normalized_liquidation_list_array(
        liquidations
            .iter()
            .map(|liq| liq.liquidations.iter().collect_vec())
            .collect_vec(),
    );

    let gas_details_array =
        get_gas_details_array(liquidations.iter().map(|liq| liq.gas_details).collect());

    let schema = Schema::new(vec![
        Field::new("liquidation_tx_hash", DataType::Utf8, false),
        Field::new("trigger", DataType::Utf8, false),
        Field::new("liquidation_swaps", liquidation_swaps_array.data_type().clone(), false),
        Field::new("liquidations", liquidations_array.data_type().clone(), false),
        Field::new("gas_details", gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(liquidation_tx_hash_array),
            Arc::new(trigger_array),
            Arc::new(liquidation_swaps_array),
            Arc::new(liquidations_array),
            Arc::new(gas_details_array),
        ],
    )
}
