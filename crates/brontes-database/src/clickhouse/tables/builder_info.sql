CREATE TABLE brontes.builder_info ON CLUSTER eth_cluster0
(
    `address` String,
    `name` Nullable(String),
    `fund` Nullable(String),
    `pub_keys` Array(String),
    `searchers_eoas` Array(String),
    `searchers_contracts` Array(String),
    `ultrasound_relay_collateral_address` Nullable(String)
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/builder_info', '{replica}')
ORDER BY `address`



