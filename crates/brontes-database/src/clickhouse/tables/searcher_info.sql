CREATE TABLE brontes.searcher_info ON CLUSTER eth_cluster0
(
    `address` String,
    `fund` LowCardinality(String),
    `mev` Array(String),
    `builder` Nullable(String),
    `eoa_or_contract` Enum8('EOA' = 0, 'Contract' = 1),
    `last_updated` UInt64 Default now()
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/searcher_info', '{replica}', `last_updated`)
ORDER BY `address`




