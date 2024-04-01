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

use crate::parquet::utils::{build_float64_array, build_string_array};

pub fn get_normalized_burn_list_array(
    normalized_burns_list: Vec<&Vec<NormalizedBurn>>,
) -> ListArray {
    let fields = vec![
        Field::new("protocol", DataType::Utf8, false),
        Field::new("trace_index", DataType::UInt16, false),
        Field::new("from", DataType::Utf8, false),
        Field::new("recipient", DataType::Utf8, false),
        Field::new("pool", DataType::Utf8, false),
        Field::new(
            "tokens",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            false,
        ),
        Field::new(
            "amounts",
            DataType::List(Arc::new(Field::new("item", DataType::Float64, true))),
            false,
        ),
    ];

    let builder_array: Vec<Box<dyn ArrayBuilder>> = vec![
        Box::new(StringBuilder::new()),
        Box::new(UInt16Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(ListBuilder::new(StringBuilder::new())),
        Box::new(ListBuilder::new(Float64Builder::new())),
    ];

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_burns in normalized_burns_list {
        let struct_builder = list_builder.values();

        for burn in normalized_burns {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(burn.protocol.to_string());

            struct_builder
                .field_builder::<UInt16Builder>(1)
                .unwrap()
                .append_value(burn.trace_index as u16);

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(burn.from.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(burn.recipient.to_string());

            struct_builder
                .field_builder::<StringBuilder>(4)
                .unwrap()
                .append_value(burn.pool.to_string());

            let token_list_array = build_string_array(
                burn.token
                    .iter()
                    .map(|token| token.address.to_string())
                    .collect_vec(),
            );

            struct_builder
                .field_builder::<ListBuilder<StringBuilder>>(5)
                .unwrap()
                .append_value(&token_list_array);

            let amount_list_array = build_float64_array(
                burn.amount
                    .iter()
                    .map(|a| a.clone().to_float())
                    .collect_vec(),
            );
            struct_builder
                .field_builder::<ListBuilder<Float64Builder>>(6)
                .unwrap()
                .append_value(&amount_list_array);

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}
