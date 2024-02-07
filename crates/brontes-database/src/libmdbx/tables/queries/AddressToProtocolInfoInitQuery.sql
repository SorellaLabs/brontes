SELECT DISTINCT 
    toString(address) AS address, 
    (arrayMap(x -> toString(x), tokens), init_block, classifier_name) AS tokens
FROM ethereum.pools 
LEFT JOIN brontes.protocol_details
on ethereum.pools.address = brontes.protocol_details.address
HAVING classifier_name IS NOT NULL AND classifier_name != ''
