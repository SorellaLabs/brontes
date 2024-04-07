use arrow::{
    array::{ArrayBuilder, ListArray, ListBuilder, StringBuilder, StructArray, StructBuilder},
    datatypes::{DataType, Field},
};
use brontes_types::tree::GasDetails;

pub fn get_gas_details_list_array(gas_details_list: Vec<&Vec<GasDetails>>) -> ListArray {
    let fields = gas_details_fields();
    let builder_array = gas_details_struct_builder();

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for gas_details_vec in gas_details_list {
        let struct_builder = list_builder.values();

        for gas_details in gas_details_vec {
            if let Some(coinbase_transfer) = gas_details.coinbase_transfer {
                struct_builder
                    .field_builder::<StringBuilder>(0)
                    .unwrap()
                    .append_value(coinbase_transfer.to_string());
            } else {
                struct_builder
                    .field_builder::<StringBuilder>(0)
                    .unwrap()
                    .append_null();
            }

            struct_builder
                .field_builder::<StringBuilder>(1)
                .unwrap()
                .append_value(gas_details.priority_fee.to_string());

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(gas_details.gas_used.to_string());

            struct_builder
                .field_builder::<StringBuilder>(3)
                .unwrap()
                .append_value(gas_details.effective_gas_price.to_string());
            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
}

pub fn get_gas_details_array(gas_details: Vec<GasDetails>) -> StructArray {
    let fields = gas_details_fields();
    let builder_array = gas_details_struct_builder();

    let mut struct_builder = StructBuilder::new(fields, builder_array);

    for gas_detail in gas_details {
        if let Some(coinbase_transfer) = gas_detail.coinbase_transfer {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(coinbase_transfer.to_string());
        } else {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_null();
        }

        struct_builder
            .field_builder::<StringBuilder>(1)
            .unwrap()
            .append_value(gas_detail.priority_fee.to_string());

        struct_builder
            .field_builder::<StringBuilder>(2)
            .unwrap()
            .append_value(gas_detail.gas_used.to_string());

        struct_builder
            .field_builder::<StringBuilder>(3)
            .unwrap()
            .append_value(gas_detail.effective_gas_price.to_string());

        struct_builder.append(true);
    }

    struct_builder.finish()
}

fn gas_details_fields() -> Vec<Field> {
    vec![
        Field::new("coinbase_transfer", DataType::Utf8, true),
        Field::new("priority_fee", DataType::Utf8, false),
        Field::new("gas_used", DataType::Utf8, false),
        Field::new("effective_gas_price", DataType::Utf8, false),
    ]
}

fn gas_details_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
    ]
}
