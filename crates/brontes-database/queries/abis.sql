SELECT 
	address, abi 
FROM brontes.protocol_details
WHERE has([?], address)

