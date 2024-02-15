SELECT
    CAST(Null, 'Nullable(String)') AS name,
    CAST([], 'Array(String)') AS pub_keys,
    CAST([], 'Array(String)') AS searchers,
    CAST(Null, 'Nullable(String)') AS ultrasound_relay_collateral_address
FROM ethereum.builders
WHERE pub_key != '' AND pub_key IS NOT NULL AND valid = 1
GROUP BY address, name
