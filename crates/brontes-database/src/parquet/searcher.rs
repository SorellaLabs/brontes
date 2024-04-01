use std::sync::Arc;

use alloy_primitives::Address;
use arrow::{
    array::{BooleanBuilder, Float64Builder, UInt64Builder},
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

    let vi_builder_array = get_string_array_from_owned(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(|info| info.1.builder.as_ref().map(|addr| addr.to_string()))
            .collect_vec(),
    );

    let mev_types_array = get_list_string_array(
        eoa_info
            .iter()
            .chain(&contract_info)
            .map(|info| &info.1.config_labels)
            .collect_vec(),
    );

    let mut is_mev_contract_builder =
        BooleanBuilder::with_capacity(eoa_info.len() + contract_info.len());
    is_mev_contract_builder.append_slice(&vec![false; eoa_info.len()]);
    is_mev_contract_builder.append_slice(&vec![true; contract_info.len()]);
    let is_mev_contract_array = is_mev_contract_builder.finish();

    // Flatten MevCount fields
    let mut bundle_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut sandwich_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut cex_dex_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut jit_count_builder = UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut jit_sandwich_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut atomic_backrun_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut liquidation_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut searcher_tx_count_builder =
        UInt64Builder::with_capacity(eoa_info.len() + contract_info.len());

    // Flatten TollByType fields for pnl and gas_bids
    let mut pnl_total_builder = Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_sandwich_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_cex_dex_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_jit_builder = Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_jit_sandwich_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_atomic_backrun_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_liquidation_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut pnl_searcher_tx_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());

    let mut gas_bids_total_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_sandwich_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_cex_dex_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_jit_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_jit_sandwich_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_atomic_backrun_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_liquidation_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());
    let mut gas_bids_searcher_tx_builder =
        Float64Builder::with_capacity(eoa_info.len() + contract_info.len());

    for info in eoa_info.iter().chain(&contract_info) {
        let mev_count = &info.1.mev_count;
        bundle_count_builder.append_value(mev_count.bundle_count);
        sandwich_count_builder.append_option(mev_count.sandwich_count);
        cex_dex_count_builder.append_option(mev_count.cex_dex_count);
        jit_count_builder.append_option(mev_count.jit_count);
        jit_sandwich_count_builder.append_option(mev_count.jit_sandwich_count);
        atomic_backrun_count_builder.append_option(mev_count.atomic_backrun_count);
        liquidation_count_builder.append_option(mev_count.liquidation_count);
        searcher_tx_count_builder.append_option(mev_count.searcher_tx_count);

        let pnl = &info.1.pnl;
        pnl_total_builder.append_value(pnl.total);
        pnl_sandwich_builder.append_option(pnl.sandwich);
        pnl_cex_dex_builder.append_option(pnl.cex_dex);
        pnl_jit_builder.append_option(pnl.jit);
        pnl_jit_sandwich_builder.append_option(pnl.jit_sandwich);
        pnl_atomic_backrun_builder.append_option(pnl.atomic_backrun);
        pnl_liquidation_builder.append_option(pnl.liquidation);
        pnl_searcher_tx_builder.append_option(pnl.searcher_tx);

        let gas_bids = &info.1.gas_bids;
        gas_bids_total_builder.append_value(gas_bids.total);
        gas_bids_sandwich_builder.append_option(gas_bids.sandwich);
        gas_bids_cex_dex_builder.append_option(gas_bids.cex_dex);
        gas_bids_jit_builder.append_option(gas_bids.jit);
        gas_bids_jit_sandwich_builder.append_option(gas_bids.jit_sandwich);
        gas_bids_atomic_backrun_builder.append_option(gas_bids.atomic_backrun);
        gas_bids_liquidation_builder.append_option(gas_bids.liquidation);
        gas_bids_searcher_tx_builder.append_option(gas_bids.searcher_tx);
    }

    let schema = Schema::new(vec![
        Field::new("address", DataType::Utf8, false),
        Field::new("is_contract", DataType::Boolean, false),
        Field::new("fund", DataType::Utf8, true),
        Field::new(
            "config_labels",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("builder", DataType::Utf8, true),
        Field::new("bundle_count", DataType::UInt64, false),
        Field::new("sandwich_count", DataType::UInt64, true),
        Field::new("cex_dex_count", DataType::UInt64, true),
        Field::new("jit_count", DataType::UInt64, true),
        Field::new("jit_sandwich_count", DataType::UInt64, true),
        Field::new("atomic_backrun_count", DataType::UInt64, true),
        Field::new("liquidation_count", DataType::UInt64, true),
        Field::new("searcher_tx_count", DataType::UInt64, true),
        Field::new("pnl_total", DataType::Float64, false),
        Field::new("pnl_sandwich", DataType::Float64, true),
        Field::new("pnl_cex_dex", DataType::Float64, true),
        Field::new("pnl_jit", DataType::Float64, true),
        Field::new("pnl_jit_sandwich", DataType::Float64, true),
        Field::new("pnl_atomic_backrun", DataType::Float64, true),
        Field::new("pnl_liquidation", DataType::Float64, true),
        Field::new("pnl_searcher_tx", DataType::Float64, true),
        Field::new("gas_bids_total", DataType::Float64, false),
        Field::new("gas_bids_sandwich", DataType::Float64, true),
        Field::new("gas_bids_cex_dex", DataType::Float64, true),
        Field::new("gas_bids_jit", DataType::Float64, true),
        Field::new("gas_bids_jit_sandwich", DataType::Float64, true),
        Field::new("gas_bids_atomic_backrun", DataType::Float64, true),
        Field::new("gas_bids_liquidation", DataType::Float64, true),
        Field::new("gas_bids_searcher_tx", DataType::Float64, true),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(address_array),
            Arc::new(is_mev_contract_array),
            Arc::new(fund_array),
            Arc::new(mev_types_array),
            Arc::new(vi_builder_array),
            Arc::new(bundle_count_builder.finish()),
            Arc::new(sandwich_count_builder.finish()),
            Arc::new(cex_dex_count_builder.finish()),
            Arc::new(jit_count_builder.finish()),
            Arc::new(jit_sandwich_count_builder.finish()),
            Arc::new(atomic_backrun_count_builder.finish()),
            Arc::new(liquidation_count_builder.finish()),
            Arc::new(searcher_tx_count_builder.finish()),
            Arc::new(pnl_total_builder.finish()),
            Arc::new(pnl_sandwich_builder.finish()),
            Arc::new(pnl_cex_dex_builder.finish()),
            Arc::new(pnl_jit_builder.finish()),
            Arc::new(pnl_jit_sandwich_builder.finish()),
            Arc::new(pnl_atomic_backrun_builder.finish()),
            Arc::new(pnl_liquidation_builder.finish()),
            Arc::new(pnl_searcher_tx_builder.finish()),
            Arc::new(gas_bids_total_builder.finish()),
            Arc::new(gas_bids_sandwich_builder.finish()),
            Arc::new(gas_bids_cex_dex_builder.finish()),
            Arc::new(gas_bids_jit_builder.finish()),
            Arc::new(gas_bids_jit_sandwich_builder.finish()),
            Arc::new(gas_bids_atomic_backrun_builder.finish()),
            Arc::new(gas_bids_liquidation_builder.finish()),
            Arc::new(gas_bids_searcher_tx_builder.finish()),
        ],
    )
}
