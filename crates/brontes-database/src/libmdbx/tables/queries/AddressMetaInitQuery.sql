WITH most_recent AS (
    SELECT
        address,
        max(last_updated) as max_updated
    FROM ethereum.addresses
    WHERE 
        entity_name IS NOT NULL OR
        nametag IS NOT NULL
    GROUP BY address
)
SELECT
    toString(a.address) AS address,
    a.entity_name,
    a.nametag,
    a.labels,
    a.type,
    (a.verified_contract, a.contract_creation_addr, a.protocol_subtype, a.reputation) AS contract_info,
    a.ens,
    (a.twitter, a.twitter_followers, a.website_url, a.crunchbase, a.linkedin) AS socials
FROM ethereum.addresses AS a
INNER JOIN most_recent AS mr ON a.address = mr.address AND a.last_updated = mr.max_updated
WHERE 
    a.entity_name IS NOT NULL OR
    a.nametag IS NOT NULL
