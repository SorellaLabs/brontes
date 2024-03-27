use std::sync::Arc;

use arrow::{
    array::{
        ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
        UInt64Builder,
    },
    datatypes::{DataType, Field},
};
use brontes_types::{normalized_actions::NormalizedMint, ToFloatNearest};
use itertools::Itertools;

use crate::parquet::utils::{build_float64_array, build_string_array};

pub fn get_normalized_mint_list_array(
    normalized_mints_list: Vec<Vec<&NormalizedMint>>,
) -> ListArray {
    let fields = normalized_mint_fields();
    let builder_array = normalized_mint_struct_builder();

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_mints_vec in normalized_mints_list {
        let struct_builder = list_builder.values();

        for normalized_mint in normalized_mints_vec {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(normalized_mint.protocol.to_string());

            struct_builder
                .field_builder::<UInt64Builder>(1)
                .unwrap()
                .append_value(normalized_mint.trace_index);

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(normalized_mint.from.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(normalized_mint.recipient.to_string());

            struct_builder
                .field_builder::<StringBuilder>(4)
                .unwrap()
                .append_value(normalized_mint.pool.to_string());

            let token_list_array = build_string_array(
                normalized_mint
                    .token
                    .iter()
                    .map(|token| token.address.to_string())
                    .collect_vec(),
            );

            struct_builder
                .field_builder::<ListBuilder<StringBuilder>>(5)
                .unwrap()
                .append_value(&token_list_array);

            let amount_list_array = build_float64_array(
                normalized_mint
                    .amount
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

fn normalized_mint_fields() -> Vec<Field> {
    vec![
        Field::new("protocol", DataType::Utf8, false),
        Field::new("trace_index", DataType::UInt64, false),
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
    ]
}

fn normalized_mint_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(UInt64Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(ListBuilder::new(StringBuilder::new())),
        Box::new(ListBuilder::new(Float64Builder::new())),
    ]
}
