use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use brontes_types::db::{
    block_times::BlockTimes,
    cex::{quotes::RawCexQuotes, BestCexPerPair, CexSymbols},
};

pub async fn fetch_test_data(
    client: &Clickhouse,
    range: CexRangeOrArbitrary,
) -> eyre::Result<(Vec<BlockTimes>, Vec<CexSymbols>, Vec<RawCexQuotes>, Vec<BestCexPerPair>)> {
    let block_times: Vec<BlockTimes> = client.get_block_times_range(&range).await?;
    let symbols = client.get_cex_symbols().await?;
    let start_time = block_times.first().unwrap().timestamp;
    let end_time = block_times.last().unwrap().timestamp + 300 * 1_000_000;

    let raw_quotes = client
        .get_raw_cex_quotes_range(start_time, end_time)
        .await?;
    let symbol_rank = client.fetch_symbol_rank(&block_times, &range).await?;

    Ok((block_times, symbols, raw_quotes, symbol_rank))
}
