CREATE TABLE brontes.token_info 
(
    `address` String,
    `symbol` String,
    `decimals` UInt8
)
ENGINE = MergeTree()
PRIMARY KEY `address`
ORDER BY `address`



