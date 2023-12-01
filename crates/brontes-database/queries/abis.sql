SELECT 
	distinct(a.address), any(c.abi)
FROM ethereum.addresses a 
INNER JOIN ethereum.contracts c ON a.hashed_bytecode = c.hashed_bytecode
WHERE c.abi IS NOT NULL AND has([?], address)

