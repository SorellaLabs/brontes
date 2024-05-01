WITH
    ? AS p2p_time,
    bids_asks AS
    (
        SELECT
            max(bt.timestamp) AS timestamp,
            bt.symbol AS symbol,
            bt.exchange AS exchange,
            any(bt.ask_price) AS ask_price,
            any(bt.bid_price) AS bid_price
        FROM cex.normalized_quotes AS bt
        WHERE (bt.timestamp <= p2p_time) AND (bt.timestamp > (p2p_time - 1000000))
        GROUP BY
            bt.symbol,
            bt.exchange
    ),
    prices_a AS
    (
        SELECT
            ba.timestamp AS timestamp,
            ba.symbol AS pair,
            s.base_asset AS base_asset,
            s.quote_asset AS quote_asset,
            et1.address AS base_address,
            et2.address AS quote_address,
            ba.ask_price AS ask_price,
            ba.bid_price AS bid_price
        FROM bids_asks AS ba
        INNER JOIN cex.symbols AS s ON (s.pair = ba.symbol) AND (ba.exchange = s.exchange)
        INNER JOIN ethereum.tokens AS et2 ON et2.symbol = s.quote_asset
        INNER JOIN ethereum.tokens AS et1 ON et1.symbol = s.base_asset
    ),
    prices_b AS
    (
        SELECT
            ba.timestamp AS timestamp,
            concat(s.quote_asset, s.base_asset) AS pair,
            s.base_asset AS base_asset,
            s.quote_asset AS quote_asset,
            toString(et2.address) AS base_address,
            toString(et1.address) AS quote_address,
            1 / ba.ask_price AS bid_price,
            1 / ba.bid_price AS ask_price
        FROM bids_asks AS ba
        INNER JOIN cex.symbols AS s ON (s.pair = ba.symbol) AND (ba.exchange = s.exchange)
        INNER JOIN ethereum.tokens AS et2 ON et2.symbol = s.quote_asset
        INNER JOIN ethereum.tokens AS et1 ON et1.symbol = s.base_asset
    ),
    prices AS
    (
        SELECT (base_address, quote_address), (timestamp, ask_price, bid_price) FROM (
            SELECT *
            FROM prices_a
            UNION ALL
            SELECT *
            FROM prices_b
        )
    )
SELECT *
FROM prices