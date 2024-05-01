use arrow::{
    array::{
        ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
        UInt16Builder,
    },
    datatypes::{DataType, Field},
};
use brontes_types::{normalized_actions::NormalizedLiquidation, ToFloatNearest};

pub fn get_normalized_liquidation_list_array(
    normalized_liquidations_list: Vec<Vec<&NormalizedLiquidation>>,
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
        Field::new("msg_value", DataType::Utf8, false),
    ];

    let builder_array: Vec<Box<dyn ArrayBuilder>> = vec![
        Box::new(StringBuilder::new()),
        Box::new(UInt16Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(StringBuilder::new()),
    ];

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_liquidations in normalized_liquidations_list {
        let struct_builder = list_builder.values();

        for liquidation in normalized_liquidations {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(liquidation.protocol.to_string());
            struct_builder
                .field_builder::<UInt16Builder>(1)
                .unwrap()
                .append_value(liquidation.trace_index as u16);

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(liquidation.pool.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(liquidation.liquidator.to_string());

            struct_builder
                .field_builder::<StringBuilder>(4)
                .unwrap()
                .append_value(liquidation.debtor.to_string());
            struct_builder
                .field_builder::<StringBuilder>(5)
                .unwrap()
                .append_value(liquidation.collateral_asset.address.to_string());

            struct_builder
                .field_builder::<StringBuilder>(6)
                .unwrap()
                .append_value(liquidation.debt_asset.address.to_string());

            struct_builder
                .field_builder::<Float64Builder>(7)
                .unwrap()
                .append_value(liquidation.covered_debt.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(8)
                .unwrap()
                .append_value(liquidation.liquidated_collateral.clone().to_float());

            struct_builder
                .field_builder::<StringBuilder>(9)
                .unwrap()
                .append_value(liquidation.msg_value.to_string());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}
