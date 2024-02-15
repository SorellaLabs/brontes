WITH most_recent AS (
    SELECT
        distinct(address)
    FROM ethereum.addresses
    WHERE 
        entity_name IS NOT NULL OR
        nametag IS NOT NULL
)
SELECT
    toString(a.address) AS address,
    a.entity_name,
    a.nametag,
    a.labels,
    CASE 
        WHEN a.arkham_type IS NOT NULL AND a.etherscan_type IS NOT NULL THEN a.arkham_type
        WHEN a.arkham_type IS NULL AND a.etherscan_type IS NOT NULL THEN a.etherscan_type
        WHEN a.arkham_type IS NOT NULL AND a.etherscan_type IS NULL THEN a.arkham_type
        ELSE NULL
    END AS type,
    (a.verified_contract, a.contract_creation_addr, a.protocol_subtype, a.reputation) AS contract_info,
    a.ens,
    (a.twitter, a.twitter_followers, a.website_url, a.crunchbase, a.linkedin) AS socials
FROM ethereum.address_meta AS a
INNER JOIN most_recent AS mr ON a.address = mr.address
WHERE 
    a.entity_name IS NOT NULL OR
    a.nametag IS NOT NULL