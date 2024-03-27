use std::sync::Arc;

use arrow::{
    array::{
        ArrayBuilder, ArrayRef, Float64Builder, ListArray, ListBuilder, StringBuilder, StructArray,
        StructBuilder, UInt16Builder, UInt64Builder,
    },
    datatypes::{DataType, Field},
};
use brontes_types::{normalized_actions::NormalizedSwap, ToFloatNearest};

pub fn get_normalized_swap_list_array(
    normalized_swaps_list: Vec<Vec<&NormalizedSwap>>,
) -> ListArray {
    let fields = fields();
    let builder_array = struct_builder();
    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_swaps in normalized_swaps_list {
        let struct_builder = list_builder.values();

        for swap in normalized_swaps {
            println!("Protocol: {}", swap.protocol.to_string());
            println!("Trace Index: {}", swap.trace_index as u16);
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(swap.protocol.to_string());

            struct_builder
                .field_builder::<UInt16Builder>(1)
                .unwrap()
                .append_value(swap.trace_index as u16);

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(swap.from.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(swap.recipient.to_string());

            struct_builder
                .field_builder::<StringBuilder>(4)
                .unwrap()
                .append_value(swap.pool.to_string());

            struct_builder
                .field_builder::<StringBuilder>(5)
                .unwrap()
                .append_value(swap.token_in.address.to_string());

            struct_builder
                .field_builder::<StringBuilder>(6)
                .unwrap()
                .append_value(swap.token_out.address.to_string());

            struct_builder
                .field_builder::<Float64Builder>(7)
                .unwrap()
                .append_value(swap.amount_in.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(8)
                .unwrap()
                .append_value(swap.amount_out.clone().to_float());

            struct_builder
                .field_builder::<UInt64Builder>(9)
                .unwrap()
                .append_value(swap.msg_value.to());
        }

        let protocol_length = list_builder
            .values()
            .field_builder::<StringBuilder>(0)
            .unwrap()
            .len();
        let trace_index_length = list_builder
            .values()
            .field_builder::<UInt16Builder>(1)
            .unwrap()
            .len();
        list_builder.append(true);
    }

    list_builder.finish()
}

#[allow(dead_code)]
fn get_normalized_swap_array(normalized_swaps: Vec<NormalizedSwap>) -> StructArray {
    let mut protocol_builder = StringBuilder::new();
    let mut trace_index_builder = UInt16Builder::new();
    let mut from_builder = StringBuilder::new();
    let mut recipient_builder = StringBuilder::new();
    let mut pool_builder = StringBuilder::new();
    let mut token_in_builder = StringBuilder::new();
    let mut token_out_builder = StringBuilder::new();

    let mut amount_in_builder = Float64Builder::new();
    let mut amount_out_builder = Float64Builder::new();
    let mut msg_value_builder = UInt64Builder::new();

    for swap in normalized_swaps {
        protocol_builder.append_value(swap.protocol.to_string());
        trace_index_builder.append_value(swap.trace_index as u16);
        from_builder.append_value(swap.from.to_string());
        recipient_builder.append_value(swap.recipient.to_string());
        pool_builder.append_value(swap.pool.to_string());
        token_in_builder.append_value(swap.token_in.address.to_string());
        token_out_builder.append_value(swap.token_out.address.to_string());
        amount_in_builder.append_value(swap.amount_in.to_float());
        amount_out_builder.append_value(swap.amount_out.to_float());
        msg_value_builder.append_value(swap.msg_value.to());
    }

    let protocol_array = protocol_builder.finish();
    let trace_index_array = trace_index_builder.finish();
    let from_array = from_builder.finish();
    let recipient_array = recipient_builder.finish();
    let pool_array = pool_builder.finish();
    let token_in_array = token_in_builder.finish();
    let token_out_array = token_out_builder.finish();
    let amount_in_array = amount_in_builder.finish();
    let amount_out_array = amount_out_builder.finish();
    let msg_value_array = msg_value_builder.finish();

    let fields = fields();

    let arrays = vec![
        Arc::new(protocol_array) as ArrayRef,
        Arc::new(trace_index_array),
        Arc::new(from_array),
        Arc::new(recipient_array),
        Arc::new(pool_array),
        Arc::new(token_in_array),
        Arc::new(token_out_array),
        Arc::new(amount_in_array),
        Arc::new(amount_out_array),
        Arc::new(msg_value_array),
    ];

    StructArray::try_new(fields.into(), arrays, None).expect("Failed to init struct arrays")
}

fn fields() -> Vec<Field> {
    vec![
        Field::new("protocol", DataType::Utf8, false),
        Field::new("trace_index", DataType::UInt16, false),
        Field::new("from", DataType::Utf8, false),
        Field::new("recipient", DataType::Utf8, false),
        Field::new("pool", DataType::Utf8, false),
        Field::new("token_in", DataType::Utf8, false),
        Field::new("token_out", DataType::Utf8, false),
        Field::new("amount_in", DataType::Float64, false),
        Field::new("amount_out", DataType::Float64, false),
        Field::new("msg_value", DataType::UInt64, false),
    ]
}

fn struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(UInt16Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(UInt64Builder::new()),
    ]
}
