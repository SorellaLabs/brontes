SELECT
    *
FROM cex.normalized_quotes 
WHERE timestamp >= ? AND timestamp < ?
