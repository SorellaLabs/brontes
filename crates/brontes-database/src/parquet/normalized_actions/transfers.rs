use arrow::{
    array::{
        ArrayBuilder, Float64Builder, ListArray, ListBuilder, StringBuilder, StructBuilder,
        UInt16Builder,
    },
    datatypes::{DataType, Field},
};
use brontes_types::{normalized_actions::NormalizedTransfer, ToFloatNearest};

pub fn get_normalized_transfer_list_array(
    normalized_transfers_list: Vec<&Vec<NormalizedTransfer>>,
) -> ListArray {
    let fields = normalized_transfer_fields();
    let builder_array = normalized_transfer_struct_builder();

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for normalized_transfers_vec in normalized_transfers_list {
        let struct_builder = list_builder.values();

        for normalized_transfer in normalized_transfers_vec {
            struct_builder
                .field_builder::<UInt16Builder>(0)
                .unwrap()
                .append_value(normalized_transfer.trace_index as u16);

            struct_builder
                .field_builder::<StringBuilder>(1)
                .unwrap()
                .append_value(normalized_transfer.from.to_string());

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(normalized_transfer.to.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(normalized_transfer.token.address.to_string());

            struct_builder
                .field_builder::<Float64Builder>(4)
                .unwrap()
                .append_value(normalized_transfer.amount.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(5)
                .unwrap()
                .append_value(normalized_transfer.fee.clone().to_float());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}

fn normalized_transfer_fields() -> Vec<Field> {
    vec![
        Field::new("trace_index", DataType::UInt16, false),
        Field::new("from", DataType::Utf8, false),
        Field::new("to", DataType::Utf8, false),
        Field::new("token", DataType::Utf8, false),
        Field::new("amount", DataType::Float64, false),
        Field::new("fee", DataType::Float64, false),
    ]
}

fn normalized_transfer_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(UInt16Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
    ]
}
