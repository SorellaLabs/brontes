use arrow::{
    array::{ArrayBuilder, Decimal128Builder, ListArray, ListBuilder, StructArray, StructBuilder},
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
            struct_builder
                .field_builder::<Decimal128Builder>(0)
                .unwrap()
                .append_option(
                    gas_details
                        .coinbase_transfer
                        .map(|value| value.try_into().expect("Gas detail overflow")),
                );

            struct_builder
                .field_builder::<Decimal128Builder>(1)
                .unwrap()
                .append_value(
                    gas_details
                        .priority_fee
                        .try_into()
                        .expect("Gas detail overflow"),
                );

            struct_builder
                .field_builder::<Decimal128Builder>(2)
                .unwrap()
                .append_value(
                    gas_details
                        .gas_used
                        .try_into()
                        .expect("Gas detail overflow"),
                );

            struct_builder
                .field_builder::<Decimal128Builder>(3)
                .unwrap()
                .append_value(
                    gas_details
                        .effective_gas_price
                        .try_into()
                        .expect("Gas detail overflow"),
                );
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
        struct_builder
            .field_builder::<Decimal128Builder>(0)
            .unwrap()
            .append_option(
                gas_detail
                    .coinbase_transfer
                    .map(|value| value.try_into().expect("Gas detail overflow")),
            );

        struct_builder
            .field_builder::<Decimal128Builder>(1)
            .unwrap()
            .append_value(
                gas_detail
                    .priority_fee
                    .try_into()
                    .expect("Gas detail overflow"),
            );

        struct_builder
            .field_builder::<Decimal128Builder>(2)
            .unwrap()
            .append_value(gas_detail.gas_used.try_into().expect("Gas detail overflow"));

        struct_builder
            .field_builder::<Decimal128Builder>(3)
            .unwrap()
            .append_value(
                gas_detail
                    .effective_gas_price
                    .try_into()
                    .expect("Gas detail overflow"),
            );

        struct_builder.append(true);
    }

    struct_builder.finish()
}

fn gas_details_fields() -> Vec<Field> {
    vec![
        Field::new("coinbase_transfer", DataType::Decimal128(38, 10), true),
        Field::new("priority_fee", DataType::Decimal128(38, 10), false),
        Field::new("gas_used", DataType::Decimal128(38, 10), false),
        Field::new("effective_gas_price", DataType::Decimal128(38, 10), false),
    ]
}

fn gas_details_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(Decimal128Builder::new()),
        Box::new(Decimal128Builder::new()),
        Box::new(Decimal128Builder::new()),
        Box::new(Decimal128Builder::new()),
    ]
}
