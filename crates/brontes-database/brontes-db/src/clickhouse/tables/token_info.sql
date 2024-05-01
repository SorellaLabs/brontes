CREATE TABLE brontes.token_info ON CLUSTER eth_cluster0
(
    `address` String,
    `symbol` String,
    `decimals` UInt8
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/token_info', '{replica}')
PRIMARY KEY `address`
ORDER BY `address`



