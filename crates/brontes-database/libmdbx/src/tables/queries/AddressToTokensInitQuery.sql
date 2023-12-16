SELECT DISTINCT 
    toString(address) AS address, 
    arrayMap(x -> toString(x), tokens) AS tokens 
FROM ethereum.pools
LIMIT 100