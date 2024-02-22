WITH 
    aggr AS (
        SELECT
            block_number,
            exchange,
            pair,
            argMinMerge(cex_timestamp) AS cex_timestamp,
            argMinMerge(ask_price) AS ask_price,
            argMinMerge(bid_price) AS bid_price
        FROM brontes.cex_pricing
        WHERE block_number = ? 
        GROUP BY
            block_number,
            exchange,
            pair
    ),
    all AS (
        SELECT 
            aggr.block_number AS block_number,
            aggr.exchange AS exchange,
            groupArray(((aggr.pair), (aggr.cex_timestamp, (aggr.ask_price, aggr.bid_price), aggr.pair.1))) AS metadata
        FROM aggr
        GROUP BY block_number, exchange
    )
SELECT
    CAST(block_number, 'UInt64') AS block_number,
    groupArray((exchange, metadata)) AS data
FROM all 
GROUP BY block_number
