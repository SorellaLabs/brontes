with 
  pools as (
    select cast(count(), 'UInt64') as pool_creation from ( select init_block from ethereum.pools group by init_block )
  ),
  address_to_protocol as (
    select cast(count(), 'UInt64') as address_to_protocol from ethereum.pools 
  ),
  tokens as (
    select cast(count(), 'UInt64') as tokens from brontes.token_info
  ),
  builder as (
    select cast(count(), 'UInt64') as builder from brontes_api.builder_info
  ),
  address_meta as(
    select cast(count(), 'UInt64') as address_meta from brontes_api.address_meta
  )
  select 
    p.*,
    a.*,
    t.*,
    b.*,
    am.* 
  from pools as p
    cross join address_to_protocol as a
    cross join tokens as t
    cross join builder as b
    cross join address_meta as am


