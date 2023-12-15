SELECT 
    toString(address) AS address, 
    decimals FROM ethereum.dex_tokens 
LIMIT 100