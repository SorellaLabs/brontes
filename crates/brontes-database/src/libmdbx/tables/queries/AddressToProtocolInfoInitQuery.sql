SELECT DISTINCT 
    toString(p.address) AS address, 
    (arrayMap(x -> toString(x), p.tokens), p.init_block, CAST(d.classifier_name, 'String')) AS tokens
FROM ethereum.pools2 p
RIGHT JOIN brontes.protocol_details d ON p.address = d.address
WHERE classifier_name IS NOT NULL

