CREATE TABLE IF NOT EXISTS timeboost.bids (
    timestamp DateTime64(3),
    chain_id UInt64,
    bidder String,
    express_lane_controller String,
    auction_contract_address String,
    round UInt64,
    amount UInt64,
    signature String
) ENGINE = ReplacingMergeTree()
PARTITION BY (chain_id, toStartOfDay(timestamp))
ORDER BY (chain_id, round, bidder)