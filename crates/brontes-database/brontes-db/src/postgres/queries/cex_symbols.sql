SELECT DISTINCT
    s.exchange AS exchange, 
    UPPER(REPLACE(REPLACE(REPLACE(s.pair, '/', ''), '-', ''), '_', '')) AS symbol_pair,
    p1.address::text AS address1,
    p2.address::text AS address2
FROM cex.trading_pairs s
INNER JOIN cex.address_symbols AS p1 ON p1.symbol = s.base_asset OR p1.unwrapped_symbol = s.base_asset
INNER JOIN cex.address_symbols AS p2 ON p2.symbol = s.quote_asset OR p2.unwrapped_symbol = s.quote_asset