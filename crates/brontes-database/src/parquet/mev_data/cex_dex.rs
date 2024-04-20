use arrow::{
    array::{ArrayBuilder, Float64Builder, ListArray, StringBuilder},
    datatypes::{DataType, Field},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::{ArbDetails, CexDex};

pub fn cex_dex_to_record_batch(cex_dex_arbs: Vec<CexDex>) -> Result<RecordBatch, ArrowError> {
    todo!()
    /*
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

    let stat_arb_details_array = get_stat_arb_details_list_array(
        cex_dex_arbs
            .iter()
            .map(|cd| &cd.stat_arb_details)
            .collect_vec(),
    );

    let maker_pnl_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|cd| cd.pnl.maker_profit.clone().to_float())
            .collect_vec(),
    );

    let taker_pnl_array = build_float64_array(
        cex_dex_arbs
            .iter()
            .map(|cd| cd.pnl.taker_profit.clone().to_float())
            .collect_vec(),
    );

    let gas_details_array =
        get_gas_details_array(cex_dex_arbs.iter().map(|cd| cd.gas_details).collect());

    let schema = Schema::new(vec![
        Field::new("tx_hash", tx_hash_array.data_type().clone(), false),
        Field::new("swaps", swaps_array.data_type().clone(), false),
        Field::new("stat_arb_details", stat_arb_details_array.data_type().clone(), false),
        Field::new("maker_pnl", DataType::Float64, false),
        Field::new("taker_pnl", DataType::Float64, false),
        Field::new("gas_details", gas_details_array.data_type().clone(), false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(tx_hash_array),
            Arc::new(swaps_array),
            Arc::new(stat_arb_details_array),
            Arc::new(maker_pnl_array),
            Arc::new(taker_pnl_array),
            Arc::new(gas_details_array),
        ],
    )*/
}

fn get_stat_arb_details_list_array(stat_arb_details_list: Vec<&Vec<ArbDetails>>) -> ListArray {
    //TODO:
    todo!();
    /*
    let fields = stat_arb_details_fields();
    let builder_array = stat_arb_details_struct_builder();

    let mut list_builder = ListBuilder::new(StructBuilder::new(fields, builder_array));

    for stat_arb_details_vec in stat_arb_details_list {
        let struct_builder = list_builder.values();

        for stat_arb_details in stat_arb_details_vec {
            struct_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(stat_arb_details.cex_exchange.to_string());

            struct_builder
                .field_builder::<Float64Builder>(1)
                .unwrap()
                .append_value(stat_arb_details.cex_price.clone().to_float());

            struct_builder
                .field_builder::<StringBuilder>(2)
                .unwrap()
                .append_value(stat_arb_details.dex_exchange.to_string());

            struct_builder
                .field_builder::<Float64Builder>(3)
                .unwrap()
                .append_value(stat_arb_details.dex_price.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(4)
                .unwrap()
                .append_value(stat_arb_details.pnl_pre_gas.maker_profit.clone().to_float());

            struct_builder
                .field_builder::<Float64Builder>(5)
                .unwrap()
                .append_value(stat_arb_details.pnl_pre_gas.taker_profit.clone().to_float());

            struct_builder.append(true);
        }

        list_builder.append(true);
    }

    list_builder.finish()
    */
}

fn stat_arb_details_fields() -> Vec<Field> {
    vec![
        Field::new("cex_exchange", DataType::Utf8, false),
        Field::new("cex_price", DataType::Float64, false),
        Field::new("dex_exchange", DataType::Utf8, false),
        Field::new("dex_price", DataType::Float64, false),
        Field::new("maker_pnl", DataType::Float64, false),
        Field::new("taker_pnl", DataType::Float64, false),
    ]
}

fn stat_arb_details_struct_builder() -> Vec<Box<dyn ArrayBuilder>> {
    vec![
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(StringBuilder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
        Box::new(Float64Builder::new()),
    ]
}
