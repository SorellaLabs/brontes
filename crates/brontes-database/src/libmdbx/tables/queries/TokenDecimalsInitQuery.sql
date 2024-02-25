SELECT address, mapFromArrays(['symbol', 'decimals'], [symbol, toString(decimals)])  FROM brontes_api.token_info
