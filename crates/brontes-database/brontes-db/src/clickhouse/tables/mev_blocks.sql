CREATE TABLE IF NOT EXISTS mev.mev_blocks 
(
    `block_hash` String,
    `block_number` UInt64,
    `mev_count` Nested (
        `mev_count` UInt64,
        `sandwich_count` UInt64,
        `cex_dex_trade_count` UInt64,
        `cex_dex_quote_count` UInt64,
        `cex_dex_rfq_count` UInt64,
        `jit_count` UInt64,
        `jit_sandwich_count` UInt64,
        `atomic_backrun_count` UInt64,
        `liquidation_count` UInt64
    ),
    `eth_price` Float64,
    `total_gas_used` UInt128,
    `total_priority_fee` UInt128,
    `total_bribe` UInt128,
    `total_mev_bribe` UInt128, 
    `total_mev_priority_fee_paid` UInt128,
    `builder_address` String,
    `builder_name` Nullable(String),
    `builder_eth_profit` Float64,
    `builder_profit_usd` Float64,
    `builder_mev_profit_usd` Float64,
    `builder_searcher_bribes` UInt128,
    `builder_searcher_bribes_usd` Float64,
    `builder_sponsorship_amount` UInt128,
    `ultrasound_bid_adjusted` Bool,
    `proposer_fee_recipient` Nullable(String),
    `proposer_mev_reward` Nullable(UInt128),
    `proposer_profit_usd` Nullable(Float64),
    `total_mev_profit_usd` Float64,
    `possible_mev` Nested (
        `tx_hash` String,
        `tx_idx` UInt64,
        `gas_details.coinbase_transfer` Nullable(UInt128), 
        `gas_details.priority_fee` UInt128,
        `gas_details.gas_used` UInt128,
        `gas_details.effective_gas_price` UInt128,
        `triggers.is_private` Bool,
        `triggers.coinbase_transfer` Bool,
        `triggers.high_priority_fee` Bool
    ),
    `run_id` UInt64
) 
ENGINE = MergeTree()
PRIMARY KEY (`block_number`, `block_hash`)
ORDER BY (`block_number`, `block_hash`)
