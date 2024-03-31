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

select arrayMap(x -> arrayFilter(l -> l.1.1 == '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48' or l.1.2 == '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', x.2),data) from brontes_api.cex_trades where block_number = 18592518 format Vertical

