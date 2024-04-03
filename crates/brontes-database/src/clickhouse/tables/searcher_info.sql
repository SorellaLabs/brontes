CREATE TABLE brontes.searcher_info ON CLUSTER eth_cluster0
(
    `address` String,
    `eoa_or_contract` Enum8('EOA' = 0, 'Contract' = 1),
    `fund` LowCardinality(String),
    `config_labels` Array(String),
    `builder` Nullable(String),
    `bundle_count` UInt64,
    `sandwich_count` Nullable(UInt64),
    `cex_dex_count` Nullable(UInt64),
    `jit_count` Nullable(UInt64),
    `jit_sandwich_count` Nullable(UInt64),
    `atomic_backrun_count` Nullable(UInt64),
    `liquidation_count` Nullable(UInt64),
    `searcher_tx_count` Nullable(UInt64),
    `pnl_total` Float64,
    `pnl_sandwich` Nullable(Float64),
    `pnl_cex_dex` Nullable(Float64),
    `pnl_jit` Nullable(Float64),
    `pnl_jit_sandwich` Nullable(Float64),
    `pnl_atomic_backrun` Nullable(Float64),
    `pnl_liquidation` Nullable(Float64),
    `pnl_searcher_tx` Nullable(Float64),
    `gas_bids_total` Float64,
    `gas_bids_sandwich` Nullable(Float64),
    `gas_bids_cex_dex` Nullable(Float64),
    `gas_bids_jit` Nullable(Float64),
    `gas_bids_jit_sandwich` Nullable(Float64),
    `gas_bids_atomic_backrun` Nullable(Float64),
    `gas_bids_liquidation` Nullable(Float64),
    `gas_bids_searcher_tx` Nullable(Float64),
    `last_updated` UInt64 DEFAULT now()
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/searcher_info', '{replica}', `last_updated`)
ORDER BY `address`




