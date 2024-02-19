SELECT
    toString(address) AS address,
    CAST(Null, 'Nullable(String)') AS name,
    CAST(Null, 'Nullable(String)') AS fund,
    CAST([], 'Array(String)') AS pub_keys,
    CAST([], 'Array(String)') AS searchers_eoas,
    CAST([], 'Array(String)') AS searchers_contracts,
    CAST(Null, 'Nullable(String)') AS ultrasound_relay_collateral_address
FROM ethereum.builders
WHERE pub_key != '' AND pub_key IS NOT NULL AND valid = 1
GROUP BY address, name
