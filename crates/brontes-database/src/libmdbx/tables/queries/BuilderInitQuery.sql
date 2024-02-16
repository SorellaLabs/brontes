SELECT
    toString(address) AS address,
    CAST(name, 'Nullable(String)') AS name,
    CAST(groupArray(pub_key), 'Array(String)') AS pub_keys,
    CAST([], 'Array(String)') AS searchers,
    CAST(Null, 'Nullable(String)') AS ultrasound_relay_collateral_address
FROM eth_analytics.builder_meta
WHERE pub_key != ''
GROUP BY address, name
