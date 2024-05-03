CREATE TABLE brontes.builder_stats ON CLUSTER eth_cluster0
(
    `address` String,
    `pnl` Float64,
    `blocks_built` UInt64,
    `last_active` UInt64,
    `last_updated` UInt64 Default now()
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/builder_stats', '{replica}', `last_updated`)
ORDER BY `address`



