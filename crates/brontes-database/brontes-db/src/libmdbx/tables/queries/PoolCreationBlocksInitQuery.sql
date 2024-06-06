select 
  init_block as block_number,
  groupArray(address) as pools
from ethereum.pools
group by block_number
