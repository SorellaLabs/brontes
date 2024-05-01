use std::sync::Arc;

use arrow::{
    array::{StringArray, StringBuilder},
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::BundleHeader;

use super::utils::{
    build_float64_array, build_record_batch, build_string_array, build_uint64_array,
};

pub fn bundle_headers_to_record_batch(
    bundle_headers: Vec<BundleHeader>,
) -> Result<RecordBatch, ArrowError> {
    let block_number_array =
        build_uint64_array(bundle_headers.iter().map(|bh| bh.block_number).collect());
    let tx_index_array: arrow::array::PrimitiveArray<arrow::datatypes::UInt64Type> =
        build_uint64_array(bundle_headers.iter().map(|bh| bh.tx_index).collect());
    let tx_hash_array = build_string_array(
        bundle_headers
            .iter()
            .map(|bh| bh.tx_hash.to_string())
            .collect(),
    );
    let eoa_array =
        build_string_array(bundle_headers.iter().map(|bh| bh.eoa.to_string()).collect());
    let mev_contract_array = get_mev_contract_array(&bundle_headers);

    let profit_usd_array =
        build_float64_array(bundle_headers.iter().map(|bh| bh.profit_usd).collect());
    let bribe_usd_array =
        build_float64_array(bundle_headers.iter().map(|bh| bh.bribe_usd).collect());
    let mev_type_array = build_string_array(
        bundle_headers
            .iter()
            .map(|bh| bh.mev_type.to_string())
            .collect(),
    );

    let schema = Schema::new(vec![
        Field::new("block_number", DataType::UInt64, false),
        Field::new("tx_index", DataType::UInt64, false),
        Field::new("tx_hash", DataType::Utf8, false),
        Field::new("eoa", DataType::Utf8, false),
        Field::new("mev_contract", DataType::Utf8, true),
        Field::new("profit_usd", DataType::Float64, false),
        Field::new("bribe_usd", DataType::Float64, false),
        Field::new("mev_type", DataType::Utf8, false),
    ]);

    build_record_batch(
        schema,
        vec![
            Arc::new(block_number_array),
            Arc::new(tx_index_array),
            Arc::new(tx_hash_array),
            Arc::new(eoa_array),
            Arc::new(mev_contract_array),
            Arc::new(profit_usd_array),
            Arc::new(bribe_usd_array),
            Arc::new(mev_type_array),
        ],
    )
}

fn get_mev_contract_array(bundle_headers: &Vec<BundleHeader>) -> StringArray {
    // Storing as string so 40
    let mev_contract_data_capacity = 40 * bundle_headers.len();
    let mut mev_contract_array =
        StringBuilder::with_capacity(bundle_headers.len(), mev_contract_data_capacity);

    for bundle in bundle_headers {
        mev_contract_array.append_option(bundle.mev_contract.as_ref().map(|addr| addr.to_string()));
    }

    mev_contract_array.finish()
}
