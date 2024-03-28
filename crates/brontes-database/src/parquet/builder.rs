use std::sync::Arc;

use alloy_primitives::Address;
use arrow::{
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::db::builder::BuilderInfo;
use itertools::Itertools;

use super::utils::{
    build_string_array, get_list_string_array_from_owned, get_string_array_from_owned,
};

pub fn builder_info_to_record_batch(
    builder_info: Vec<(Address, BuilderInfo)>,
) -> Result<RecordBatch, ArrowError> {
    let address_array = build_string_array(
        builder_info
            .iter()
            .map(|info| info.0.to_string())
            .collect_vec(),
    );

    let fund_array =
        get_string_array_from_owned(builder_info.iter().map(|info| info.1.fund).collect_vec());

    let pub_keys_array = get_list_string_array_from_owned(
        builder_info
            .iter()
            .map(|info| info.1.pub_keys.iter().map(|s| s.to_string()).collect_vec())
            .collect_vec(),
    );

    let searchers_eoa_array = get_list_string_array_from_owned(
        builder_info
            .iter()
            .map(|info| {
                info.1
                    .searchers_eoas
                    .iter()
                    .map(|s| s.to_string())
                    .collect_vec()
            })
            .collect_vec(),
    );

    let searchers_contract_array = get_list_string_array_from_owned(
        builder_info
            .iter()
            .map(|info| {
                info.1
                    .searchers_contracts
                    .iter()
                    .map(|s| s.to_string())
                    .collect_vec()
            })
            .collect_vec(),
    );

    let ultrasound_relay_address_array = get_string_array_from_owned(
        builder_info
            .iter()
            .map(|info| {
                info.1
                    .ultrasound_relay_collateral_address
                    .as_ref()
                    .map(|addr| addr.to_string())
            })
            .collect_vec(),
    );

    let schema = Schema::new(vec![
        Field::new("address", DataType::Utf8, false),
        Field::new("fund", DataType::Utf8, true),
        Field::new(
            "pub keys",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new(
            "searcher_eoas",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new(
            "searcher_contracts",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("collateral_addr", DataType::Utf8, true),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(address_array),
            Arc::new(fund_array),
            Arc::new(pub_keys_array),
            Arc::new(searchers_eoa_array),
            Arc::new(searchers_contract_array),
            Arc::new(ultrasound_relay_address_array),
        ],
    )
}
