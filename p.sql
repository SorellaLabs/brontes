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




SELECT
    exchange,
    upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    timestamp,
    side,
    if (side == 'buy', price, divide(1, price)) AS price,
    if (side == 'buy', amount, multiply(divide(1, price), amount)) AS amount
FROM cex.normalized_trades 
WHERE timestamp >= 1693067309000 AND timestamp < 1693067381000 AND (exchange = 'binance' OR exchange = 'coinbase' OR exchange = 'okex' OR exchange = 'bybit-spot' OR exchange = 'kucoin')
