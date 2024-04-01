SELECT
    *
FROM cex.normalized_trades 
WHERE timestamp >= ? AND timestamp < ?
