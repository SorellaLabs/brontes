CREATE TABLE IF NOT EXISTS brontes_api.builder_info 
(
    `address` String,
    `name` String,
    `fund` String,
    `pub_keys` Array(String),
    `searchers_eoas` Array(String),
    `searchers_contracts` Array(String),
    `ultrasound_relay_collateral_address` String,
)
ENGINE = MergeTree()
PRIMARY KEY address
ORDER BY address;