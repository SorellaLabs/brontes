with (
  select block_timestamp * 1000000 from ethereum.blocks where block_number = 18264690
) as h
select count() from 
cex.normalized_trades 
where timestamp >= h - 5000000 
  and timestamp < h + 8000000 
  and upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) = 'PERLUSDT'
