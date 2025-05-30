CREATE TABLE IF NOT EXISTS timeboost.bids (
    timestamp DateTime64(3),
    chain_id UInt64,
    bidder String,
    express_lane_controller String,
    auction_contract_address String,
    round UInt64,
    amount String,
    signature String
) ENGINE = MergeTree()
PARTITION BY chain_id
PRIMARY KEY (timestamp)
ORDER BY (timestamp)