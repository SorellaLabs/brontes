SELECT DISTINCT 
    toString(address) AS address, 
    (arrayMap(x -> toString(x), tokens), init_block) AS tokens
FROM ethereum.pools