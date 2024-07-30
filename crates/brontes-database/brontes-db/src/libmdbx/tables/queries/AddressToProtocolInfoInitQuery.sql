SELECT
    cast(address,'String') as address,
    cast(CASE
      WHEN protocol = 'Curve.fi' AND protocol_subtype = 'Base' THEN (tokens, init_block, concat(protocol, protocol_subtype, toString(length(tokens))), curve_lp_token)
      ELSE (tokens, init_block, concat(protocol, protocol_subtype), curve_lp_token)
    END, 'Tuple(Array(String),UInt64,String,Nullable(String))') AS tokens
FROM ethereum.pools 
WHERE length(pools.tokens) >= 2