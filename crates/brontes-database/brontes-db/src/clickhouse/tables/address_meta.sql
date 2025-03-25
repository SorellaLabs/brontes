CREATE TABLE brontes_api.address_meta (
    `address` String,
    `entity_name` String,
    `nametag` String,
    `labels` String,
    `type` String,
    `contract_info` String,
    `ens` String,
    `socials` String
) 
ENGINE=MergeTree()
PRIMARY KEY address
ORDER BY address;