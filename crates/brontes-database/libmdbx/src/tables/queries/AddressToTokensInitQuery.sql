SELECT DISTINCT 
    toString(address) AS address, 
    arrayMap(x -> toString(x), tokens) AS tokens 
FROM ethereum.pools
WHERE address = '0x9aaaaacb0aa06a0f7b16da0361610fa7ebd2c62a'
LIMIT 100