pub const PRICES: &str = r#"SELECT 
    substring(toString(sub1.address), 3) AS address,
    sub1.price AS relay_price,
    sub2.price AS p2p_price
FROM
(
    SELECT 
        max(bt.timestamp) as timestamp, 
        et.address as address, 
        round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
    FROM 
        cex.binance_idv_symbol_tickers as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp < ?)
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) as sub1 INNER JOIN (
    SELECT 
        max(bt.timestamp) as timestamp, 
        et.address as address, 
        round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
    FROM 
        cex.binance_idv_symbol_tickers as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp < ?)
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) AS sub2 ON sub2.address = sub1.address


"#;

pub const PRIVATE_FLOW: &str = r#"SELECT toString(tx_hash) as tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM ethereum.blocks
    WHERE (block_number = ?) AND (block_hash = ?) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM ethereum.unique_mempool
)


"#;

pub const RELAY_P2P_TIMES: &str = r#"SELECT
    max(relays.timestamp) AS relay_timestamp,
    toUInt64(round(max(cb.timestamp) / 1000)) AS p2p_timestamp,
    any(toString(relays.fee_recipient)) AS proposer_addr,
    any(relays.value) AS proposer_reward
FROM ethereum.relays 
INNER JOIN ethereum.block_observations as cb
ON ethereum.relays.block_number = cb.block_number
WHERE (block_number = ?) AND (block_hash = ?)

"#;
