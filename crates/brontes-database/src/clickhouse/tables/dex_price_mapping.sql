CREATE TABLE brontes.dex_price_mapping ON CLUSTER eth_cluster0
(
    `block_number` UInt64,
    `tx_idx` UInt64,
    `quote` Array(Tuple(Tuple(String, String), Tuple(Tuple(Array(UInt64), Array(UInt64)), Tuple(Array(UInt64), Array(UInt64))))),
    `last_updated` UInt64
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/dex_price_mapping', '{replica}', `last_updated`)
Order By (`block_number`, `tx_idx`)
