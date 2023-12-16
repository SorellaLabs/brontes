SELECT toString(tx_hash) as tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM ethereum.blocks
    WHERE (block_number = ?) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM ethereum.unique_mempool
)




WITH
    18000000 AS block_number,
    private_flow AS (
            SELECT toString(bt.tx_hash) as tx_hash
            FROM
            (
                SELECT toString(arrayJoin(transaction_hashes)) AS tx_hash
                FROM ethereum.blocks
                WHERE (blocks.block_number = block_number) AND (valid = 1)
            ) AS bt
            WHERE bt.tx_hash NOT IN (
                SELECT DISTINCT um.tx_hash
                FROM ethereum.unique_mempool AS um
                WHERE um.tx_hash IN (
                    SELECT toString(arrayJoin(transaction_hashes)) AS tx_hash
                    FROM ethereum.blocks
                    WHERE (blocks.block_number = block_number) AND (valid = 1)
                )
            )
        ),
        grouped_private_flow AS (
        SELECT 
            groupArray(tx_hash) AS private_flow
        FROM private_flow
    )
SELECT
    length(grouped_private_flow.private_flow),
    grouped_private_flow.private_flow
FROM grouped_private_flow




WITH
    18000000 AS block_number,
    private_flow AS
    (
        SELECT groupArray(a.tx_hash)
        FROM
        (
            SELECT DISTINCT arrayJoin(transaction_hashes) AS tx_hash
            FROM ethereum.blocks
            WHERE (blocks.block_number = block_number) AND (valid = 1)
        ) AS a
        LEFT JOIN
        (
            SELECT a.tx_hash AS tx_hash
            FROM ethereum.unique_mempool AS um
            INNER JOIN
            (
                SELECT DISTINCT arrayJoin(transaction_hashes) AS tx_hash
                FROM ethereum.blocks
                WHERE (blocks.block_number = block_number) AND (valid = 1)
            ) AS a ON a.tx_hash = um.tx_hash
        ) AS pub ON a.tx_hash = pub.tx_hash
        WHERE (pub.tx_hash = '') OR (pub.tx_hash IS NULL)
    )
SELECT *
FROM private_flow



SELECT sub.tx_hash FROM (
    SELECT distinct(arrayJoin(b.transaction_hashes)) AS tx_hash
    FROM ethereum.blocks b
    WHERE (b.block_number = 18000000) AND (b.valid = 1)
) sub
LEFT JOIN ethereum.unique_mempool AS um ON sub.tx_hash = um.tx_hash
WHERE um.tx_hash IS NULL OR um.tx_hash = ''



SELECT sub.tx_hash as tx FROM ethereum.unique_mempool AS um
RIGHT JOIN (
    SELECT distinct(arrayJoin(b.transaction_hashes)) AS tx_hash
    FROM ethereum.blocks b
    WHERE (b.block_number = 18000000) AND (b.valid = 1)
) sub ON sub.tx_hash = um.tx_hash
WHERE um.tx_hash IS NULL OR um.tx_hash = ''
ORDER BY tx
