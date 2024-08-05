WITH
    block_times AS (
        SELECT
            timestamp_add('seconds', $1, block_timestamp) AS block_start,
            timestamp_add('seconds', $2, block_timestamp) AS block_end
        FROM ethereum.blocks
        WHERE block_number = ANY(UNNEST($3::BIGINT[]))
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
    WHERE c.exchange = ANY(UNNEST($4::STRING[]))