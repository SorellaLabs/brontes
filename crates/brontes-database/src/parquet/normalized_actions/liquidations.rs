use std::sync::Arc;

use arrow::{
    array::{
        ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
        UInt16Builder,
    },
    datatypes::{DataType, Field},
};
use brontes_types::{normalized_actions::NormalizedBurn, ToFloatNearest};
use itertools::Itertools;

use crate::parquet::utils::{build_float64_array, get_string_array_from_owned};

fn get_normalized_liquidation_list_array(
    normalized_liquidations_list: &[Vec<NormalizedLiquidation>],
) -> ListArray {
    let fields = vec![
        Field::new("protocol", DataType::Utf8, false),
        Field::new("trace_index", DataType::UInt16, false),
        Field::new("pool", DataType::Utf8, false),
        Field::new("liquidator", DataType::Utf8, false),
        Field::new("debtor", DataType::Utf8, false),
        Field::new("collateral_asset", DataType::Utf8, false),
        Field::new("debt_asset", DataType::Utf8, false),
        Field::new("covered_debt", DataType::Float64, false),
        Field::new("liquidated_collateral", DataType::Float64, false),
        Field::new("msg_value", DataType::Float64, false),
    ];

    let builder_array = vec![
        Box::new(StringBuilder::new()),
        Box::new(UInt16Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
    ];

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_liquidations in normalized_liquidations_list {
        let struct_builder = list_builder.values();

        let protocol_array = struct_builder.field_builder::<StringBuilder>(0).unwrap();
        let trace_index_array = struct_builder.field_builder::<UInt16Builder>(1).unwrap();
        let pool_array = struct_builder.field_builder::<StringBuilder>(2).unwrap();
        let liquidator_array = struct_builder.field_builder::<StringBuilder>(3).unwrap();
        let debtor_array = struct_builder.field_builder::<StringBuilder>(4).unwrap();
        let collateral_asset_array = struct_builder.field_builder::<StringBuilder>(5).unwrap();
        let debt_asset_array = struct_builder.field_builder::<StringBuilder>(6).unwrap();
        let covered_debt_array = struct_builder.field_builder::<Float64Builder>(7).unwrap();
        let liquidated_collateral_array =
            struct_builder.field_builder::<Float64Builder>(8).unwrap();
        let msg_value_array = struct_builder.field_builder::<Float64Builder>(9).unwrap();

        for liquidation in normalized_liquidations {
            protocol_array.append_value(liquidation.protocol.to_string());
            trace_index_array.append_value(liquidation.trace_index as u16);
            pool_array.append_value(liquidation.pool.to_string());
            liquidator_array.append_value(liquidation.liquidator.to_string());
            debtor_array.append_value(liquidation.debtor.to_string());
            collateral_asset_array.append_value(liquidation.collateral_asset.address.to_string());
            debt_asset_array.append_value(liquidation.debt_asset.address.to_string());
            covered_debt_array.append_value(liquidation.covered_debt.to_float());
            liquidated_collateral_array.append_value(liquidation.liquidated_collateral.to_float());
            msg_value_array.append_value(liquidation.msg_value.as_u64() as f64);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}
