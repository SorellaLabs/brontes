select 
  cast(init_block, 'UInt64') as block_number,
  cast(groupArray(address), 'Array(String)') as pools
from ethereum.pools
group by block_number
