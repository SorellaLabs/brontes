use arrow::{
    array::{
        Array, ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
    },
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::{
    db::cex::CexExchange,
    mev::{cex_dex::ArbPnl, ArbDetails, CexDex},
    ToFloatNearest,
};
use itertools::Itertools;
use polars::prelude::*;

use crate::parquet::{
    normalized_actions::{
        gas_details::get_gas_details_array, swaps::get_normalized_swap_list_array,
    },
    utils::{build_float64_array, get_string_array_from_owned},
};

pub fn cex_dex_to_record_batch(cex_dex_arbs: Vec<CexDex>) -> Result<RecordBatch, ArrowError> {
    let tx_hash_array = get_string_array_from_owned(
        cex_dex_arbs
            .iter()
            .map(|cd| Some(cd.tx_hash.to_string()))
            .collect_vec(),
    );

    let swaps_array = get_normalized_swap_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| cd.swaps.iter().collect_vec())
            .collect_vec(),
    );

    let or_maker_mid_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.optimal_route_pnl.maker_taker_mid.0.clone().to_float())
            .collect(),
    );

    let or_taker_mid_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.optimal_route_pnl.maker_taker_mid.1.clone().to_float())
            .collect(),
    );

    let or_maker_ask_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.optimal_route_pnl.maker_taker_ask.0.clone().to_float())
            .collect(),
    );

    let or_taker_ask_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.optimal_route_pnl.maker_taker_ask.1.clone().to_float())
            .collect(),
    );

    let g_maker_mid_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.global_vmap_pnl.maker_taker_mid.0.clone().to_float())
            .collect(),
    );

    let g_taker_mid_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.global_vmap_pnl.maker_taker_mid.1.clone().to_float())
            .collect(),
    );

    let g_maker_ask_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.global_vmap_pnl.maker_taker_ask.0.clone().to_float())
            .collect(),
    );

    let g_taker_ask_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|bh| bh.global_vmap_pnl.maker_taker_ask.1.clone().to_float())
            .collect(),
    );

    let or_arb_details_array = get_stat_arb_details_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| &cd.optimal_route_details)
            .collect_vec(),
    );

    let g_arb_details_array = get_stat_arb_details_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| &cd.global_vmap_details)
            .collect_vec(),
    );

    let exchange_arb_details_deep_list = get_deep_stat_arb_details_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| &cd.per_exchange_details)
            .collect_vec(),
    );

    let exchanges_pnl_array = get_cex_exchange_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| &cd.per_exchange_pnl)
            .collect_vec(),
    );

    let gas_details_array =
        get_gas_details_array(cex_dex_arbs.iter().map(|cd| cd.gas_details).collect());

    let schema = Schema::new(vec![
        Field::new("tx_hash", tx_hash_array.data_type().clone(), false),
        Field::new("swaps", swaps_array.data_type().clone(), false),
        Field::new("or_maker_mid_pnl", DataType::Float64, false),
        Field::new("or_taker_mid_pnl", DataType::Float64, false),
        Field::new("or_maker_ask_pnl", DataType::Float64, false),
        Field::new("or_taker_ask_pnl", DataType::Float64, false),
        Field::new("g_maker_mid_pnl", DataType::Float64, false),
        Field::new("g_taker_mid_pnl", DataType::Float64, false),
        Field::new("g_maker_ask_pnl", DataType::Float64, false),
        Field::new("g_taker_ask_pnl", DataType::Float64, false),
        Field::new("global_vmap_arb_details", g_arb_details_array.data_type().clone(), false),
        Field::new("optimal_route_arb_details", or_arb_details_array.data_type().clone(), false),
        Field::new(
            "exchanges_arb_details",
            exchange_arb_details_deep_list.data_type().clone(),
            false,
        ),
        Field::new("exchanges_pnl", exchanges_pnl_array.data_type().clone(), false),
        Field::new("gas_details", gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(tx_hash_array),
            Arc::new(swaps_array),
            Arc::new(or_maker_mid_array),
            Arc::new(or_taker_mid_array),
            Arc::new(or_maker_ask_array),
            Arc::new(or_taker_ask_array),
            Arc::new(g_maker_mid_array),
            Arc::new(g_taker_mid_array),
            Arc::new(g_maker_ask_array),
            Arc::new(g_taker_ask_array),
            Arc::new(gas_details_array),
        ],
    )
}

fn get_stat_arb_details_list_array(arb_details_list: Vec<&Vec<ArbDetails>>) -> ListArray {
    let fields = arb_details_fields();
    let builder_array = arb_details_struct_builder();

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for arb_details_vec in arb_details_list {
        let struct_builder = list_builder.values();

        for arb_details in arb_details_vec {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(arb_details.cex_exchange.to_string());

            struct_builder
                .field_builder::<Float64Builder>(1)
                .unwrap()
                .append_value(arb_details.best_bid_maker.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(2)
                .unwrap()
                .append_value(arb_details.best_ask_maker.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(3)
                .unwrap()
                .append_value(arb_details.best_bid_taker.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(4)
                .unwrap()
                .append_value(arb_details.best_ask_taker.clone().to_float());

            struct_builder
                .field_builder::<StringBuilder>(5)
                .unwrap()
                .append_value(arb_details.dex_exchange.to_string());

            struct_builder
                .field_builder::<Float64Builder>(6)
                .unwrap()
                .append_value(arb_details.dex_price.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(7)
                .unwrap()
                .append_value(arb_details.pnl_pre_gas.maker_taker_mid.0.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(8)
                .unwrap()
                .append_value(arb_details.pnl_pre_gas.maker_taker_mid.1.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(9)
                .unwrap()
                .append_value(arb_details.pnl_pre_gas.maker_taker_ask.0.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(10)
                .unwrap()
                .append_value(arb_details.pnl_pre_gas.maker_taker_ask.1.clone().to_float());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}

fn arb_details_fields() -> Vec<Field> {
    vec![
        Field::new("cex_exchange", DataType::Utf8, false),
        Field::new("best_bid_maker", DataType::Float64, false),
        Field::new("best_ask_maker", DataType::Float64, false),
        Field::new("best_bid_taker", DataType::Float64, false),
        Field::new("best_ask_taker", DataType::Float64, false),
        Field::new("dex_exchange", DataType::Utf8, false),
        Field::new("dex_price", DataType::Float64, false),
        Field::new("maker_mid_pnl", DataType::Float64, false),
        Field::new("taker_mid_pnl", DataType::Float64, false),
        Field::new("maker_ask_pnl", DataType::Float64, false),
        Field::new("taker_ask_pnl", DataType::Float64, false),
    ]
}

fn arb_details_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
    ]
}

fn get_deep_stat_arb_details_list_array(
    arb_details_deep_list: Vec<&Vec<Vec<ArbDetails>>>,
) -> ListArray {
    let mut outer_list_builder = ListBuilder::new(ListBuilder::new(StructBuilder::new(
        arb_details_fields(),
        arb_details_struct_builder(),
    )));

    for exchange_arb_details in arb_details_deep_list {
        let middle_list_builder = outer_list_builder.values();

        for arb_details_vec in exchange_arb_details {
            let struct_builder = middle_list_builder.values();

            for arb_details in arb_details_vec {
                struct_builder
                    .field_builder::<StringBuilder>(0)
                    .unwrap()
                    .append_value(arb_details.cex_exchange.to_string());

                struct_builder
                    .field_builder::<Float64Builder>(1)
                    .unwrap()
                    .append_value(arb_details.best_bid_maker.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(2)
                    .unwrap()
                    .append_value(arb_details.best_ask_maker.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(3)
                    .unwrap()
                    .append_value(arb_details.best_bid_taker.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(4)
                    .unwrap()
                    .append_value(arb_details.best_ask_taker.clone().to_float());

                struct_builder
                    .field_builder::<StringBuilder>(5)
                    .unwrap()
                    .append_value(arb_details.dex_exchange.to_string());

                struct_builder
                    .field_builder::<Float64Builder>(6)
                    .unwrap()
                    .append_value(arb_details.dex_price.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(7)
                    .unwrap()
                    .append_value(arb_details.pnl_pre_gas.maker_taker_mid.0.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(8)
                    .unwrap()
                    .append_value(arb_details.pnl_pre_gas.maker_taker_mid.1.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(9)
                    .unwrap()
                    .append_value(arb_details.pnl_pre_gas.maker_taker_ask.0.clone().to_float());

                struct_builder
                    .field_builder::<Float64Builder>(10)
                    .unwrap()
                    .append_value(arb_details.pnl_pre_gas.maker_taker_ask.1.clone().to_float());

                struct_builder.append(true);
            }
            middle_list_builder.append(true);
        }
        outer_list_builder.append(true);
    }
    outer_list_builder.finish()
}

fn exchange_arb_pnl_fields() -> Vec<Field> {
    vec![
        Field::new("exchange", DataType::Utf8, false),
        Field::new("maker_mid", DataType::Float64, false),
        Field::new("taker_mid", DataType::Float64, false),
        Field::new("maker_ask", DataType::Float64, false),
        Field::new("taker_ask", DataType::Float64, false),
    ]
}

fn exchange_arb_pnl_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
    ]
}

fn get_cex_exchange_list_array(
    exchange_arb_details: Vec<&Vec<(CexExchange, ArbPnl)>>,
) -> ListArray {
    let mut list_builder = ListBuilder::new(StructBuilder::new(
        exchange_arb_pnl_fields(),
        exchange_arb_pnl_struct_builder(),
    ));

    for exchange_arb_detail in exchange_arb_details {
        let struct_builder = list_builder.values();

        for (exchange, arb_pnl) in exchange_arb_detail {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(exchange.to_string());
            struct_builder
                .field_builder::<Float64Builder>(1)
                .unwrap()
                .append_value(arb_pnl.maker_taker_mid.clone().0.to_float());
            struct_builder
                .field_builder::<Float64Builder>(2)
                .unwrap()
                .append_value(arb_pnl.maker_taker_mid.1.clone().to_float());
            struct_builder
                .field_builder::<Float64Builder>(3)
                .unwrap()
                .append_value(arb_pnl.maker_taker_ask.0.clone().to_float());
            struct_builder
                .field_builder::<Float64Builder>(4)
                .unwrap()
                .append_value(arb_pnl.maker_taker_ask.1.clone().to_float());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}
