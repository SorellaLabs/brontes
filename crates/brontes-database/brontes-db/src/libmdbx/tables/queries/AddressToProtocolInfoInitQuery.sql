SELECT
    cast(address,'String') as address,
    (tokens, init_block, concat(protocol, protocol_subtype), curve_lp_token) AS tokens
FROM ethereum.pools

