SELECT
    toString(address) AS address,
    CAST(NULL, 'Nullable(String)') AS name,
    CAST(NULL, 'Nullable(String)') AS fund,
    CAST([], 'Array(String)') AS pub_keys,
    CAST([], 'Array(String)') AS searchers_eoas,
    CAST([], 'Array(String)') AS searchers_contracts,
    CAST(NULL, 'Nullable(String)') AS ultrasound_relay_collateral_address
FROM eth_analytics.builder_meta
WHERE (pub_key != '') AND (pub_key IS NOT NULL)
GROUP BY
    address,
    name