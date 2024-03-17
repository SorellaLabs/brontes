use std::{fs::File, sync::Arc};

use arrow::{
    array::{
        ArrayRef, Decimal128Builder, Float64Array, Float64Builder, ListArray, StringArray,
        StringBuilder, UInt64Array, UInt64Builder,
    },
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use brontes_types::{
    db::mev_block::MevBlockWithClassified,
    mev::{jit_sandwich, BundleHeader, MevBlock},
};
use itertools::Itertools;
use parquet::{
    arrow::arrow_writer::ArrowWriter, basic::Compression, file::properties::WriterProperties,
};

// Assuming BundleHeader, B256, Address, MevType are defined elsewhere

fn bundle_headers_to_record_batch(
    bundle_headers: Vec<BundleHeader>,
) -> Result<RecordBatch, ArrowError> {
    let block_numbers: Vec<u64> = bundle_headers.iter().map(|bh| bh.block_number).collect();
    let tx_indices: Vec<u64> = bundle_headers.iter().map(|bh| bh.tx_index).collect();
    let tx_hashes: Vec<String> = bundle_headers
        .iter()
        .map(|bh| bh.tx_hash.to_string())
        .collect();
    let eoas: Vec<String> = bundle_headers.iter().map(|bh| bh.eoa.to_string()).collect();
    let mev_contracts: Vec<Option<String>> = bundle_headers
        .iter()
        .map(|bh| bh.mev_contract.as_ref().map(|addr| addr.to_string()))
        .collect();
    let profit_usds: Vec<f64> = bundle_headers.iter().map(|bh| bh.profit_usd).collect();
    let bribe_usds: Vec<f64> = bundle_headers.iter().map(|bh| bh.bribe_usd).collect();
    let mev_types: Vec<String> = bundle_headers
        .iter()
        .map(|bh| bh.mev_type.to_string())
        .collect();

    let block_number_array = UInt64Array::from(block_numbers);
    let tx_index_array = UInt64Array::from(tx_indices);
    let tx_hash_array = StringArray::from(tx_hashes);
    let eoa_array = StringArray::from(eoas);
    let mev_contract_array = StringArray::from_iter(mev_contracts); // This now correctly handles Option<String>
    let profit_usd_array = Float64Array::from(profit_usds);
    let bribe_usd_array = Float64Array::from(bribe_usds);
    let mev_type_array = StringArray::from(mev_types);

    // Gas-related fields converted to Arrow arrays
    let gas_used_array = UInt128Array::from(gas_used);
    let priority_fee_array = UInt128Array::from(priority_fee);
    let total_bribe_array = UInt128Array::from(total_bribes);
    let cumulative_mev_priority_fee_paid_array =
        UInt128Array::from(cumulative_mev_priority_fee_paids);

    // Profits and rewards
    let builder_eth_profits_array = Float64Array::from(builder_eth_profits);
    let builder_usd_profits_array = Float64Array::from(builder_usd_profits);
    let builder_mev_profits_array = Float64Array::from(builder_mev_profits);
    let cumulative_mev_profit_usds_array = Float64Array::from(cumulative_mev_profit_usds);

    let schema = Schema::new(vec![
        Field::new("block_number", DataType::UInt64, false),
        Field::new("tx_index", DataType::UInt64, false),
        Field::new("tx_hash", DataType::Utf8, false),
        Field::new("eoa", DataType::Utf8, false),
        Field::new("mev_contract", DataType::Utf8, true), // Correctly indicates nullable
        Field::new("profit_usd", DataType::Float64, false),
        Field::new("bribe_usd", DataType::Float64, false),
        Field::new("mev_type", DataType::Utf8, false),
    ]);

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(block_number_array) as ArrayRef,
            Arc::new(tx_index_array),
            Arc::new(tx_hash_array),
            Arc::new(eoa_array),
            Arc::new(mev_contract_array),
            Arc::new(profit_usd_array),
            Arc::new(bribe_usd_array),
            Arc::new(mev_type_array),
        ],
    )
}

fn mev_block_to_record_batch(mev_blocks: Vec<MevBlock>) -> Result<RecordBatch, ArrowError> {
    let block_hash_strs: Vec<String> = mev_blocks
        .iter()
        .map(|mb| mb.block_hash.to_string())
        .collect();
    let block_numbers: Vec<u64> = mev_blocks.iter().map(|mb| mb.block_number).collect_vec();
    let eth_prices: Vec<f64> = mev_blocks.iter().map(|mb| mb.eth_price).collect_vec();

    // Gas
    let gas_used: Vec<u128> = mev_blocks
        .iter()
        .map(|mb| mb.cumulative_gas_used)
        .collect_vec();
    let priority_fee: Vec<u128> = mev_blocks
        .iter()
        .map(|mb| mb.cumulative_priority_fee)
        .collect_vec();
    let total_bribes: Vec<u128> = mev_blocks.iter().map(|mb| mb.total_bribe).collect();
    let cumulative_mev_priority_fee_paids: Vec<u128> = mev_blocks
        .iter()
        .map(|mb| mb.cumulative_mev_priority_fee_paid)
        .collect();

    // Builder Info
    let builder_addresses: Vec<String> = mev_blocks
        .iter()
        .map(|mb| mb.builder_address.to_string())
        .collect();
    let builder_eth_profits: Vec<f64> = mev_blocks
        .iter()
        .map(|mb| mb.builder_eth_profit)
        .collect_vec();
    let builder_usd_profits: Vec<f64> = mev_blocks
        .iter()
        .map(|mb| mb.builder_profit_usd)
        .collect_vec();
    let builder_mev_profits: Vec<f64> = mev_blocks
        .iter()
        .map(|mb| mb.builder_mev_profit_usd)
        .collect_vec();
    let cumulative_mev_profit_usds: Vec<f64> = mev_blocks
        .iter()
        .map(|mb| mb.cumulative_mev_profit_usd)
        .collect();

    // Mev count TODO: Transform into function
    let mut mev_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut sandwich_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut liquidation_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut atomic_arb_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut cex_dex_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut jit_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut jit_sandwich_count_builder = UInt64Builder::with_capacity(mev_blocks.len());
    let mut searcher_tx_count_builder = UInt64Builder::with_capacity(mev_blocks.len());

    //TODO: Double check string with cap params

    let mut proposer_fee_recipient_builder =
        StringBuilder::with_capacity(builder_addresses[0].len(), mev_blocks.len());
    let mut proposer_profit_usd_builder = Float64Builder::with_capacity(mev_blocks.len());

    for block in mev_blocks.iter() {
        proposer_fee_recipient_builder.append_option(
            block
                .proposer_fee_recipient
                .as_ref()
                .map(|addr| addr.to_string()),
        );

        proposer_profit_usd_builder.append_option(block.proposer_profit_usd);

        mev_count_builder.append_value(block.mev_count.mev_count);
        sandwich_count_builder.append_option(block.mev_count.sandwich_count);
        liquidation_count_builder.append_option(block.mev_count.liquidation_count);
        atomic_arb_count_builder.append_option(block.mev_count.atomic_backrun_count);
        cex_dex_count_builder.append_option(block.mev_count.cex_dex_count);
        jit_count_builder.append_option(block.mev_count.jit_count);
        jit_sandwich_count_builder.append_option(block.mev_count.jit_sandwich_count);
        searcher_tx_count_builder.append_option(block.mev_count.searcher_tx_count);
    }

    let proposer_fee_recipient_array = proposer_fee_recipient_builder.finish();
    let proposer_mev_reward_array: Vec<u128> = mev_blocks
        .iter()
        .map(|mb| mb.proposer_mev_reward.unwrap_or_default())
        .collect_vec();
    let proposer_profit_usd_array = proposer_profit_usd_builder.finish();

    let mev_count_array = mev_count_builder.finish();
    let sandwich_count_array = sandwich_count_builder.finish();
    let liquidation_count_array = liquidation_count_builder.finish();
    let atomic_arb_count_array = atomic_arb_count_builder.finish();
    let cex_dex_count_array = cex_dex_count_builder.finish();
    let jit_count_array = jit_count_builder.finish();
    let jit_sandwich_count_array = jit_sandwich_count_builder.finish();
    let searcher_tx_count_array = searcher_tx_count_builder.finish();

    // Create Arrow arrays
    let block_number_array = UInt64Array::from(block_numbers);
    let eth_price_array = Float64Array::from(eth_prices);
    let block_hash_array = StringArray::from(block_hash_strs);
    let builder_address_array = StringArray::from(builder_addresses);

    // Define schema including all fields
    let schema = Schema::new(vec![
        Field::new("block_hash", DataType::Utf8, false),
        Field::new("block_number", DataType::UInt64, false),
        Field::new("eth_price", DataType::Float64, false),
        Field::new("gas_used", DataType::UInt128, false),
        Field::new("priority_fee", DataType::UInt128, false),
        Field::new("total_bribe", DataType::UInt128, false),
        Field::new("cumulative_mev_priority_fee_paid", DataType::UInt128, false),
        Field::new("builder_address", DataType::Utf8, false),
        Field::new("builder_eth_profit", DataType::Float64, false),
        Field::new("builder_usd_profit", DataType::Float64, false),
        Field::new("builder_mev_profit_usd", DataType::Float64, false),
        Field::new("proposer_fee_recipient", DataType::Utf8, true),
        Field::new("proposer_mev_reward", DataType::UInt128, true),
        Field::new("proposer_profit_usd", DataType::Float64, true),
        Field::new("cumulative_mev_profit_usd", DataType::Float64, false),
        Field::new("mev_count", DataType::UInt64, false),
        // Include fields for sandwich_count, liquidation_count, etc., as well
        Field::new("sandwich_count", DataType::UInt64, true),
        Field::new("liquidation_count", DataType::UInt64, true),
        Field::new("atomic_arb_count", DataType::UInt64, true),
        Field::new("cex_dex_count", DataType::UInt64, true),
        Field::new("jit_count", DataType::UInt64, true),
        Field::new("jit_sandwich_count", DataType::UInt64, true),
        Field::new("searcher_tx_count", DataType::UInt64, true),
        // Add a field for possible_mevs once it's implemented
    ]);

    // Construct the RecordBatch with all fields included
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(block_hash_array),
            Arc::new(block_number_array),
            Arc::new(eth_price_array),
            Arc::new(gas_used_array),
            Arc::new(priority_fee_array),
            Arc::new(total_bribes_array),
            Arc::new(cumulative_mev_priority_fee_paids_array),
            Arc::new(builder_address_array),
            Arc::new(builder_eth_profits_array),
            Arc::new(builder_usd_profits_array),
            Arc::new(builder_mev_profits_array),
            Arc::new(proposer_fee_recipient_array),
            Arc::new(proposer_mev_reward_array),
            Arc::new(proposer_profit_usd_array),
            Arc::new(cumulative_mev_profit_usds_array),
            Arc::new(mev_count_array),
            // Continue adding arrays for optional fields...
            Arc::new(sandwich_count_array),
            Arc::new(liquidation_count_array),
            Arc::new(atomic_arb_count_array),
            Arc::new(cex_dex_count_array),
            Arc::new(jit_count_array),
            Arc::new(jit_sandwich_count_array),
            Arc::new(searcher_tx_count_array),
        ],
    )
}

fn write_to_parquet(
    record_batch: RecordBatch,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Your existing write_to_parquet implementation
    let file = File::create(file_path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(file, record_batch.schema(), Some(props))?;

    writer.write(&record_batch)?;
    writer.close()?;
    Ok(())
}

// Main function or logic to call these functions
async fn export_mev_blocks_and_bundles(
    mev_blocks_with_classified: Vec<MevBlockWithClassified>,
) -> eyre::Result<()> {
    for mev_block_with_classified in mev_blocks_with_classified {
        let block_batch = mev_block_to_record_batch(vec![mev_block_with_classified.block])?;
        let bundle_batch = bundles_to_record_batch(mev_block_with_classified.mev)?;

        // Assuming a dynamic file naming strategy based on block numbers or other
        // identifiers
        write_to_parquet(block_batch, "path/to/block_table.parquet")?;
        write_to_parquet(bundle_batch, "path/to/bundle_table.parquet")?;
    }

    Ok(())
}

fn awrite_to_parquet(
    record_batch: RecordBatch,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(file_path)?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = ArrowWriter::try_new(file, record_batch.schema(), Some(props)).unwrap();

    writer.write(&record_batch).expect("Writing batch");

    writer.close().unwrap();

    Ok(())
}
