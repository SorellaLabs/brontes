SELECT
    toString(address) AS address,
    name,
    groupArray(toString(pub_key)) AS pub_keys,
    CAST([], 'Array(String)') AS searchers,
    CAST(Null, 'Nullable(String)') AS ultrasound_relay_collateral_address
FROM ethereum.builders
GROUP BY address, name