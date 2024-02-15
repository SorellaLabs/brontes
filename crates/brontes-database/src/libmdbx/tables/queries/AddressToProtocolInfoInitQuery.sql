SELECT DISTINCT 
    toString(d.address) AS address, 
    (arrayMap(x -> toString(x), p.tokens), p.init_block, CAST(d.classifier_name, 'String'), CAST(p.curve_lp_token, 'Nullable(String)')) AS tokens
FROM ethereum.pools p
INNER JOIN brontes.protocol_details d ON p.address = d.address
WHERE classifier_name IS NOT NULL

