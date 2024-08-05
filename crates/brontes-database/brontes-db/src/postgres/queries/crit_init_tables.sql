WITH 
  pools AS (
    SELECT COUNT(*)::BIGINT AS pool_creation FROM (SELECT DISTINCT init_block FROM ethereum.pools) subquery
  ),
  address_to_protocol AS (
    SELECT COUNT(DISTINCT address)::BIGINT AS address_to_protocol 
    FROM ethereum.pools 
    WHERE CARDINALITY(pools.tokens) >= 2
  ),
  tokens AS (
    SELECT COUNT(DISTINCT address)::BIGINT AS tokens FROM brontes.token_info
  ),
  builder AS (
    SELECT COUNT(*)::BIGINT AS builder FROM brontes_api.builder_info
  ),
  address_meta AS (
    SELECT COUNT(*)::BIGINT AS address_meta FROM brontes_api.address_meta
  )
SELECT 
  p.*,
  a.*,
  t.*,
  b.*,
  am.* 
FROM pools AS p
  CROSS JOIN address_to_protocol AS a
  CROSS JOIN tokens AS t
  CROSS JOIN builder AS b
  CROSS JOIN address_meta AS am
  