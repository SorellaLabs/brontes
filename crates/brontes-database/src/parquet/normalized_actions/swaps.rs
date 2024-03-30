use arrow::{
    array::{
        ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
        UInt16Builder,
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
                .field_builder::<StringBuilder>(7)
                .unwrap()
                .append_value(&swap.token_in.symbol);

            struct_builder
                .field_builder::<StringBuilder>(8)
                .unwrap()
                .append_value(&swap.token_out.symbol);

            struct_builder
                .field_builder::<Float64Builder>(9)
                .unwrap()
                .append_value(swap.amount_in.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(10)
                .unwrap()
                .append_value(swap.amount_out.clone().to_float());

            struct_builder
                .field_builder::<StringBuilder>(11)
                .unwrap()
                .append_value(swap.msg_value.to_string());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
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
        Field::new("token_in_symbol", DataType::Utf8, false),
        Field::new("token_out_symbol", DataType::Utf8, false),
        Field::new("amount_in", DataType::Float64, false),
        Field::new("amount_out", DataType::Float64, false),
        Field::new("msg_value", DataType::Utf8, false),
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
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(StringBuilder::new()),
    ]
}
