use std::path::PathBuf;

use brontes_types::mev::{bundle::Bundle, Mev};

use eyre::Result;
use itertools::Itertools;

use polars::prelude::*;
use ratatui::{widgets::*};


use tracing_subscriber::{
    Layer,
};

use crate::get_symbols_from_transaction_accounting;

// Function to convert a Vec<Bundle> to a Polars DataFrame
pub fn bundles_to_dataframe(bundles: Vec<Bundle>) -> Result<DataFrame> {
    let mut block_numbers = Vec::new();
    let mut tx_indexes = Vec::new();
    let mut mev_types = Vec::new();
    let mut symbols = Vec::new();
    let mut protocols = Vec::new();
    let mut eoas = Vec::new();
    let mut mev_contracts = Vec::new();
    let mut profits_usd = Vec::new();
    let mut bribes_usd = Vec::new();

    for bundle in bundles.iter() {
        block_numbers.push(bundle.header.block_number);
        tx_indexes.push(bundle.header.tx_index);
        mev_types.push(bundle.header.mev_type.to_string());
        symbols.push(get_symbols_from_transaction_accounting!(&bundle.header.balance_deltas)); // Assuming this macro/functionality
        protocols.push(
            bundle
                .data
                .protocols()
                .iter()
                .map(|p| p.to_string())
                .sorted()
                .join(", "),
        );
        eoas.push(bundle.header.eoa.to_string());
        mev_contracts.push(
            bundle
                .header
                .mev_contract
                .as_ref()
                .map(|address| address.to_string())
                .unwrap_or_else(|| "Not an Mev Contract".to_string()),
        );
        profits_usd.push(bundle.header.profit_usd);
        bribes_usd.push(bundle.header.bribe_usd);
    }

    let df = DataFrame::new(vec![
        Series::new("Block Number", &block_numbers),
        Series::new("Tx Index", &tx_indexes),
        Series::new("MEV Type", &mev_types),
        Series::new("Symbols", &symbols),
        Series::new("Protocols", &protocols),
        Series::new("EOA", &eoas),
        Series::new("MEV Contract", &mev_contracts),
        Series::new("Profit USD", &profits_usd),
        Series::new("Bribe USD", &bribes_usd),
    ])?;

    Ok(df)
}

// Function to convert a DataFrame to a Vec<Row> for the Table widget
pub fn dataframe_to_table_rows(df: &DataFrame) -> Vec<Row> {
    let height = 1;
    let bottom_margin = 0;

    let num_rows = df.height();
    let mut rows = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let mut cells = Vec::new();
        for series in df.get_columns() {
            let value_str = series.get(i).unwrap().to_string();
            cells.push(Cell::from(value_str));
        }
        rows.push(Row::new(cells).height(height).bottom_margin(bottom_margin));
    }

    rows
}

pub fn get_config_dir() -> PathBuf {
    let directory = PathBuf::from(".").join("config");
    tracing::info!("Config directory: {:?}", directory);
    directory
}

#[macro_export]
macro_rules! get_symbols_from_transaction_accounting {
    ($data:expr) => {{
        use brontes_types::{db::token_info::TokenInfoWithAddress, hasher::FastHashSet};

        let mut token_info_with_addresses: Vec<TokenInfoWithAddress> = Vec::new();
        for transaction in $data {
            for address_delta in &transaction.address_deltas {
                for token_delta in &address_delta.token_deltas {
                    token_info_with_addresses.push(token_delta.token.clone());
                }
            }
        }
        let mut symbols = FastHashSet::default();
        let unique_symbols: Vec<String> = token_info_with_addresses
            .iter()
            .filter_map(|x| {
                let symbol = x.inner.symbol.to_string();
                if symbols.insert(symbol.clone()) {
                    Some(symbol)
                } else {
                    None
                }
            })
            .collect();

        unique_symbols.join(", ")
    }};
}
