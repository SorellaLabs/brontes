use std::sync::Arc;

use arrow::{
    array::{
        Array, ArrayRef, Float64Array, Float64Builder, StringArray, StringBuilder, StructArray,
        UInt64Builder,
    },
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::mev::MevBlock;

use super::utils::{
    build_float64_array, build_record_batch, build_string_array, build_uint64_array,
    u128_to_binary_array,
};

pub fn mev_block_to_record_batch(mev_blocks: Vec<MevBlock>) -> Result<RecordBatch, ArrowError> {
    let block_hash_array = build_string_array(
        mev_blocks
            .iter()
            .map(|mb| mb.block_hash.to_string())
            .collect(),
    );
    let block_number_array =
        build_uint64_array(mev_blocks.iter().map(|mb| mb.block_number).collect());
    let eth_price_array = build_float64_array(mev_blocks.iter().map(|mb| mb.eth_price).collect());

    let gas_used_array =
        u128_to_binary_array(mev_blocks.iter().map(|mb| mb.cumulative_gas_used).collect());
    let priority_fee_array = u128_to_binary_array(
        mev_blocks
            .iter()
            .map(|mb| mb.cumulative_priority_fee)
            .collect(),
    );
    let total_bribe_array =
        u128_to_binary_array(mev_blocks.iter().map(|mb| mb.total_bribe).collect());
    let cumulative_mev_priority_fee_paid_array = u128_to_binary_array(
        mev_blocks
            .iter()
            .map(|mb| mb.cumulative_mev_priority_fee_paid)
            .collect(),
    );

    let builder_address_array = build_string_array(
        mev_blocks
            .iter()
            .map(|mb| mb.builder_address.to_string())
            .collect(),
    );
    let builder_eth_profits_array =
        build_float64_array(mev_blocks.iter().map(|mb| mb.builder_eth_profit).collect());
    let builder_usd_profits_array =
        build_float64_array(mev_blocks.iter().map(|mb| mb.builder_profit_usd).collect());
    let builder_mev_profits_array = build_float64_array(
        mev_blocks
            .iter()
            .map(|mb| mb.builder_mev_profit_usd)
            .collect(),
    );
    let cumulative_mev_profit_usds_array = build_float64_array(
        mev_blocks
            .iter()
            .map(|mb| mb.cumulative_mev_profit_usd)
            .collect(),
    );

    let proposer_mev_reward_array = u128_to_binary_array(
        mev_blocks
            .iter()
            .map(|mb| mb.proposer_mev_reward.unwrap_or_default())
            .collect(),
    );

    let mev_count_array = get_mev_count_array(&mev_blocks);
    let (proposer_fee_recipient_array, proposer_profit_usd_array) =
        get_proposer_arrays(&mev_blocks);

    let schema = build_schema(&mev_count_array);

    build_record_batch(
        schema,
        vec![
            Arc::new(block_hash_array),
            Arc::new(block_number_array),
            Arc::new(mev_count_array),
            Arc::new(eth_price_array),
            Arc::new(gas_used_array),
            Arc::new(priority_fee_array),
            Arc::new(total_bribe_array),
            Arc::new(cumulative_mev_priority_fee_paid_array),
            Arc::new(builder_address_array),
            Arc::new(builder_eth_profits_array),
            Arc::new(builder_usd_profits_array),
            Arc::new(builder_mev_profits_array),
            Arc::new(proposer_fee_recipient_array),
            Arc::new(proposer_mev_reward_array),
            Arc::new(proposer_profit_usd_array),
            Arc::new(cumulative_mev_profit_usds_array),
        ],
    )
}

fn build_schema(mev_count_array: &StructArray) -> Schema {
    Schema::new(vec![
        Field::new("block_hash", DataType::Utf8, false),
        Field::new("block_number", DataType::UInt64, false),
        Field::new("mev_count", mev_count_array.data_type().clone(), false),
        Field::new("eth_price", DataType::Float64, false),
        Field::new("cumulative_gas_used", DataType::Binary, false),
        Field::new("cumulative_priority_fee", DataType::Binary, false),
        Field::new("total_bribe", DataType::Binary, false),
        Field::new("cumulative_mev_priority_fee_paid", DataType::Binary, false),
        Field::new("builder_address", DataType::Utf8, false),
        Field::new("builder_eth_profit", DataType::Float64, false),
        Field::new("builder_profit_usd", DataType::Float64, false),
        Field::new("builder_mev_profit_usd", DataType::Float64, false),
        Field::new("proposer_fee_recipient", DataType::Utf8, true),
        Field::new("proposer_mev_reward", DataType::Binary, true),
        Field::new("proposer_profit_usd", DataType::Float64, true),
        Field::new("cumulative_mev_profit_usd", DataType::Float64, false),
    ])
}

fn get_mev_count_array(mev_blocks: &Vec<MevBlock>) -> StructArray {
    let mut mev_count_builder = UInt64Builder::new();
    let mut sandwich_count_builder = UInt64Builder::new();
    let mut liquidation_count_builder = UInt64Builder::new();
    let mut atomic_backrun_count_builder = UInt64Builder::new();
    let mut cex_dex_count_builder = UInt64Builder::new();
    let mut jit_count_builder = UInt64Builder::new();
    let mut jit_sandwich_count_builder = UInt64Builder::new();
    let mut searcher_tx_count_builder = UInt64Builder::new();

    for block in mev_blocks {
        mev_count_builder.append_value(block.mev_count.bundle_count);
        sandwich_count_builder.append_option(block.mev_count.sandwich_count);
        liquidation_count_builder.append_option(block.mev_count.liquidation_count);
        atomic_backrun_count_builder.append_option(block.mev_count.atomic_backrun_count);
        cex_dex_count_builder.append_option(block.mev_count.cex_dex_count);
        jit_count_builder.append_option(block.mev_count.jit_count);
        jit_sandwich_count_builder.append_option(block.mev_count.jit_sandwich_count);
        searcher_tx_count_builder.append_option(block.mev_count.searcher_tx_count);
    }

    let mev_count_array = mev_count_builder.finish();
    let sandwich_count_array = sandwich_count_builder.finish();
    let liquidation_count_array = liquidation_count_builder.finish();
    let atomic_backrun_count_array = atomic_backrun_count_builder.finish();
    let cex_dex_count_array = cex_dex_count_builder.finish();
    let jit_count_array = jit_count_builder.finish();
    let jit_sandwich_count_array = jit_sandwich_count_builder.finish();
    let searcher_tx_count_array = searcher_tx_count_builder.finish();

    let fields = vec![
        Field::new("mev_count", DataType::UInt64, false),
        Field::new("sandwich_count", DataType::UInt64, true),
        Field::new("liquidation_count", DataType::UInt64, true),
        Field::new("atomic_backrun_count", DataType::UInt64, true),
        Field::new("cex_dex_count", DataType::UInt64, true),
        Field::new("jit_count", DataType::UInt64, true),
        Field::new("jit_sandwich_count", DataType::UInt64, true),
        Field::new("searcher_tx_count", DataType::UInt64, true),
    ];

    let arrays = vec![
        Arc::new(mev_count_array) as ArrayRef,
        Arc::new(sandwich_count_array) as ArrayRef,
        Arc::new(liquidation_count_array) as ArrayRef,
        Arc::new(atomic_backrun_count_array) as ArrayRef,
        Arc::new(cex_dex_count_array) as ArrayRef,
        Arc::new(jit_count_array) as ArrayRef,
        Arc::new(jit_sandwich_count_array) as ArrayRef,
        Arc::new(searcher_tx_count_array) as ArrayRef,
    ];

    StructArray::try_new(fields.into(), arrays, None).expect("Failed to init struct arrays")
}

fn get_proposer_arrays(mev_blocks: &Vec<MevBlock>) -> (StringArray, Float64Array) {
    let fee_recipient_data_capacity = mev_blocks[0].builder_address.len() * mev_blocks.len();
    let mut proposer_fee_recipient_builder =
        StringBuilder::with_capacity(mev_blocks.len(), fee_recipient_data_capacity);
    let mut proposer_profit_usd_builder = Float64Builder::with_capacity(mev_blocks.len());

    for block in mev_blocks {
        proposer_fee_recipient_builder.append_option(
            block
                .proposer_fee_recipient
                .as_ref()
                .map(|addr| addr.to_string()),
        );
        proposer_profit_usd_builder.append_option(block.proposer_profit_usd);
    }

    (proposer_fee_recipient_builder.finish(), proposer_profit_usd_builder.finish())
}
