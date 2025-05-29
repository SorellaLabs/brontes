CREATE TABLE timeboost.bids (
    chain_id UInt64,
    bidder String,
    express_lane_controller String,
    auction_contract_address String,
    round UInt64,
    amount String,
    signature String
) ENGINE = MergeTree()
PARTITION BY chain_id
PRIMARY KEY (round)
ORDER BY (round)