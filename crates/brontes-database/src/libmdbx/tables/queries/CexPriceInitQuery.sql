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
        GROUP BY
            block_number,
            exchange,
            pair
        WHERE block_number >= ? AND block_number < ?
    ),
    all AS (
        SELECT 
            aggr.block_number AS block_number,
            aggr.pair AS pair,
            (aggr.exchange, aggr.cex_timestamp, (aggr.ask_price, aggr.bid_price), aggr.pair.1) AS metadata
        FROM aggr
    )
SELECT
    block_number,
    groupArray((pair, metadata)) AS data
FROM all 
GROUP BY block_number