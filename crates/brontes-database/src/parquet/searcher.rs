use std::sync::Arc;

use alloy_primitives::Address;
use arrow::{
    array::BooleanBuilder,
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::db::searcher::{Fund, SearcherInfo};
use itertools::Itertools;

use super::utils::{build_string_array, get_list_string_array, get_string_array_from_owned};

pub fn searcher_info_to_record_batch(
    eoa_info: Vec<(Address, SearcherInfo)>,
    contract_info: Vec<(Address, SearcherInfo)>,
) -> Result<RecordBatch, ArrowError> {
    let address_array = build_string_array(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(|info| info.0.to_string())
            .collect_vec(),
    );

    let fund_array = get_string_array_from_owned(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(
                |info| {
                    if info.1.fund == Fund::None {
                        None
                    } else {
                        Some(info.1.fund.to_string())
                    }
                },
            )
            .collect_vec(),
    );

    let mev_types_array = get_list_string_array(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(|info| &info.1.mev)
            .collect_vec(),
    );

    let vi_builder_array = get_string_array_from_owned(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(|info| info.1.builder.as_ref().map(|addr| addr.to_string()))
            .collect_vec(),
    );

    let mut is_mev_contract_builder =
        BooleanBuilder::with_capacity(eoa_info.len() + contract_info.len());

    is_mev_contract_builder.append_slice(&vec![false; eoa_info.len()]);
    is_mev_contract_builder.append_slice(&vec![true; contract_info.len()]);

    let is_mev_contract_array = is_mev_contract_builder.finish();

    let schema = Schema::new(vec![
        Field::new("address", DataType::Utf8, false),
        Field::new("fund", DataType::Utf8, true),
        Field::new(
            "mev_types",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("builder", DataType::Utf8, true),
        Field::new("is_contract", DataType::Boolean, false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(address_array),
            Arc::new(fund_array),
            Arc::new(mev_types_array),
            Arc::new(vi_builder_array),
            Arc::new(is_mev_contract_array),
        ],
    )
}
