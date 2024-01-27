SELECT 
    toString(address) AS address, 
    (decimals, name) AS info
FROM ethereum.dex_tokens