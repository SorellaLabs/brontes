CREATE TABLE brontes.searcher_stats ON CLUSTER eth_cluster0
(
    `address` String,
    `pnl` Float64,
    `total_bribed` Float64,
    `bundle_count` UInt64,
    `last_active` UInt64,
    `last_updated` UInt64 Default now()
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/searcher_stats', '{replica}', `last_updated`)
ORDER BY `address`




