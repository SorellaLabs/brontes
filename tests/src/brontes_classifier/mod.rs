use alloy_sol_macro::sol;

mod classified_tree;
mod decoding;

mod normalized_actions;

sol!(UniswapV2, "../crates/brontes-classifier/abis/UniswapV2.json");
sol!(UniswapV3, "../crates/brontes-classifier/abis/UniswapV3.json");
