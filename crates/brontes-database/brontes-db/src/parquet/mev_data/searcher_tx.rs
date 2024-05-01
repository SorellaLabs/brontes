use std::sync::Arc;

use arrow::{
    array::Array,
    datatypes::{Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::SearcherTx;
use itertools::Itertools;

use crate::parquet::{
    normalized_actions::{
        gas_details::get_gas_details_array, transfers::get_normalized_transfer_list_array,
    },
    utils::get_string_array_from_owned,
};

pub fn searcher_tx_to_record_batch(
    searcher_txs: Vec<SearcherTx>,
) -> Result<RecordBatch, ArrowError> {
    let tx_hash_array = get_string_array_from_owned(
        searcher_txs
            .iter()
            .map(|tx| Some(tx.tx_hash.to_string()))
            .collect_vec(),
    );

    let transfers_array = get_normalized_transfer_list_array(
        searcher_txs.iter().map(|tx| &tx.transfers).collect_vec(),
    );

    let gas_details_array =
        get_gas_details_array(searcher_txs.iter().map(|tx| tx.gas_details).collect());

    let schema = Schema::new(vec![
        Field::new("tx_hash", tx_hash_array.data_type().clone(), false),
        Field::new("transfers", transfers_array.data_type().clone(), false),
        Field::new("gas_details", gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![Arc::new(tx_hash_array), Arc::new(transfers_array), Arc::new(gas_details_array)],
    )
}
