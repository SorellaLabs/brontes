SELECT
    s.exchange AS exchange,
    upper(replaceAll(replaceAll(replaceAll(s.pair, '/', ''), '-', ''), '_', '')) AS symbol_pair,
    (toString(et1.address), toString(et2.address)) AS address_pair
FROM cex.symbols s 
INNER JOIN ethereum.tokens AS et2 ON (upper(et2.symbol) = upper(s.quote_asset) OR concat('W', upper(s.quote_asset)) = upper(et2.symbol))
INNER JOIN ethereum.tokens AS et1 ON upper(et1.symbol) = upper(s.base_asset) OR concat('W', upper(s.base_asset)) = upper(et1.symbol)