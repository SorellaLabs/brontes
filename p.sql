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

select arrayMap(x -> arrayFilter(l -> l.1.1 == lower('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2') or l.1.2 == lower('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599'), x.2),data) from brontes_api.cex_trades where block_number = 19556789

