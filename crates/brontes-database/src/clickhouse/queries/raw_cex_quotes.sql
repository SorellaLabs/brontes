SELECT
    *
FROM cex.normalized_quotes 
WHERE timestamp >= ? AND timestamp < ?
LIMIT 10