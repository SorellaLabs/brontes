SELECT DISTINCT
    s.exchange AS exchange,
    upper(replaceAll(replaceAll(replaceAll(s.pair, '/', ''), '-', ''), '_', '')) AS symbol_pair,
    (toString(p1.address), toString(p2.address)) AS address_pair
FROM cex.trading_pairs s
INNER JOIN cex.address_symbols AS p1 ON p1.symbol = s.base_asset OR p1.unwrapped_symbol = s.base_asset
INNER JOIN cex.address_symbols AS p2 ON p2.symbol = s.quote_asset OR p2.unwrapped_symbol = s.quote_asset
and not (s.exchange = 'okex' and s.trading_type = 'FUTURES')
