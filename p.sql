CREATE MATERIALIZED VIEW brontes_api.cex_trades_mb ON CLUSTER eth_cluster0
TO brontes_api.cex_trades AS
WITH 
    fir AS (
        SELECT
            block_number,
            exchange,
            pair,
            groupArray((exchange, price, amount)) AS meta
        FROM brontes.cex_trades
        GROUP BY
            block_number,
            exchange,
            pair
    ),
    sec AS (
        SELECT
            block_number,
            exchange,
            groupArray((pair, meta)) AS trades
        FROM fir
        GROUP BY
            block_number,
            exchange
    )
SELECT
    block_number,
    groupArray((exchange, trades)),
    now()
FROM sec
GROUP BY block_number

