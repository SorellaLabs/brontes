SELECT 
    (toString(min(address)),
    toString(max(address)))
FROM ethereum.dex_tokens
