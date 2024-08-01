WITH
    block_times AS (
        SELECT
            block_timestamp::TIMESTAMP - make_interval(secs => $1) AS block_start,
            block_timestamp::TIMESTAMP + make_interval(secs => $2) AS block_end
        FROM ethereum.blocks
        WHERE block_number = ANY($3)
    )
    SELECT
        c.exchange AS exchange,
        'Taker' AS trade_type,
        UPPER(REPLACE(REPLACE(REPLACE(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
        c.timestamp AS timestamp,
        c.side AS side,
        c.price AS price,
        c.amount AS amount
    FROM cex.normalized_trades AS c
    INNER JOIN block_times AS bt
    ON bt.block_start <= c.timestamp AND c.timestamp <= bt.block_end
    WHERE c.exchange = ANY($4);
