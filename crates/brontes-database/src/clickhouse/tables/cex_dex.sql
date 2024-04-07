CREATE TABLE mev.cex_dex ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `swaps.trace_idx` Array(UInt64),
    `swaps.from` Array(String),
    `swaps.recipient` Array(String),
    `swaps.pool` Array(String),
    `swaps.token_in` Array(String),
    `swaps.token_out` Array(String),
    `swaps.amount_in` Array(Tuple(UInt256, UInt256)),
    `swaps.amount_out` Array(Tuple(UInt256, UInt256)),
    `stat_arb_details.cex_exchange` Array(String),
    `stat_arb_details.cex_price` Array(Tuple(UInt256, UInt256)),
    `stat_arb_details.dex_exchange` Array(String),
    `stat_arb_details.dex_price` Array(Tuple(UInt256, UInt256)),
    `stat_arb_details.pre_gas_maker_profit` Array(Tuple(UInt256, UInt256)),
    `stat_arb_details.pre_gas_taker_profit` Array(Tuple(UInt256, UInt256)),
    `pnl` Tuple(taker_profit Tuple(UInt256, UInt256), maker_profit Tuple(UInt256, UInt256)),
    `gas_details` Tuple(coinbase_transfer Nullable(UInt128), priority_fee UInt128, gas_used UInt128, effective_gas_price UInt128),
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/cex_dex', '{replica}', `last_updated`)
PRIMARY KEY (`tx_hash`)
ORDER BY (`tx_hash`)

