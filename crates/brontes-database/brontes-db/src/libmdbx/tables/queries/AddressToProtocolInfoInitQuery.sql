SELECT
    cast(address,'String') as address,
    cast((tokens, init_block, concat(protocol, protocol_subtype), curve_lp_token),
      'Tuple(Array(String),UInt64,String,Nullable(String))')
      AS tokens
FROM ethereum.pools

