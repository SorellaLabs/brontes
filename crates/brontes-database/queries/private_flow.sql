SELECT toString(tx_hash) as tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM ethereum.blocks
    WHERE (block_number = ?) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM ethereum.unique_mempool
)





SELECT toString(tx_hash) as tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM ethereum.blocks
    WHERE (block_number = 18180900) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM ethereum.unique_mempool
)


WITH
    relay_block AS
    (
        SELECT
            relays.block_number AS block_number,
            toString(blocks.block_hash) AS block_hash,
            relays.timestamp AS relay_time,
            toString(relays.proposer_fee_recipient) AS proposer_addr,
            relays.value AS proposer_reward
        FROM ethereum.relays
        INNER JOIN ethereum.blocks AS blocks ON (blocks.block_number = 18180900) AND (blocks.block_hash = ethereum.relays.block_hash) AND (blocks.valid = 1)
        WHERE relays.block_number = 18180900
    ),
    relay_p2p AS
    (
        SELECT
            any(relay_block.block_hash) AS block_hash,
            any(relay_block.relay_time) AS relay_time,
            toUInt64(round(max(cb.timestamp) / 1000)) AS p2p_time,
            any(relay_block.proposer_addr) AS proposer_addr,
            any(relay_block.proposer_reward) AS proposer_reward
        FROM relay_block
        INNER JOIN ethereum.block_observations AS cb ON relay_block.block_number = cb.block_number
    ),
    relay_prices AS (
        SELECT 
            max(bt.timestamp) as timestamp, 
            et.address as address, 
            round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
        FROM 
            relay_p2p,
            cex.normalized_quotes as bt
        INNER JOIN ethereum.tokens AS et 
        ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
        WHERE 
            (
                (bt.timestamp <= relay_p2p.relay_time) AND (bt.timestamp > relay_p2p.relay_time - 1000000)
            )
            AND substring(bt.symbol, -4) = 'USDT'
        GROUP BY 
            address
    ),
    p2p_prices AS (
        SELECT 
            max(bt.timestamp) as timestamp, 
            et.address as address, 
            round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
        FROM 
            relay_p2p,
            cex.normalized_quotes as bt
        INNER JOIN ethereum.tokens AS et 
        ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
        WHERE 
            (
                (bt.timestamp <= relay_p2p.p2p_time) AND (bt.timestamp > relay_p2p.p2p_time - 1000000)
            )
            AND substring(bt.symbol, -4) = 'USDT'
        GROUP BY 
            address
    ),
    prices AS (
        SELECT 
            substring(toString(r.address), 3) AS address,
            r.price AS relay_price,
            p.price AS p2p_price
        FROM relay_prices r
        INNER JOIN p2p_prices p ON r.address = p.address
    )
SELECT * FROM relay_p2p
SETTINGS distributed_product_mode = 'allow'