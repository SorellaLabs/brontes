use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use brontes_types::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::{
        block_times::BlockTimes,
        cex::{
            quotes::{CexQuote, CexQuotesConverter, RawCexQuotes},
            BestCexPerPair, CexExchange, CexSymbols,
        },
    },
    pair::Pair,
};
use malachite::Rational;
use tokio::runtime::Runtime;

fn setup_runtime_and_client() -> (Runtime, Clickhouse) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let client = rt.block_on(Clickhouse::new_default(None));

    (rt, client)
}

async fn fetch_test_data(
    client: &Clickhouse,
    range: CexRangeOrArbitrary,
) -> eyre::Result<(Vec<BlockTimes>, Vec<CexSymbols>, Vec<RawCexQuotes>, Vec<BestCexPerPair>)> {
    let block_times = client.get_block_times_range(&range).await?;
    let symbols = client.get_cex_symbols().await?;
    let start_time = block_times.first().unwrap().timestamp;
    let end_time = block_times.last().unwrap().timestamp + 300 * 1_000_000;

    let raw_quotes = client
        .get_raw_cex_quotes_range(start_time, end_time)
        .await?;
    let symbol_rank = client.fetch_symbol_rank(&block_times, &range).await?;

    Ok((block_times, symbols, raw_quotes, symbol_rank))
}

#[test]
fn test_cex_quote_conversion() {
    let (rt, client) = setup_runtime_and_client();
    let range = CexRangeOrArbitrary::Range(18264694, 18264795);
    let (block_times, symbols, quotes, best_cex_per_pair) =
        rt.block_on(async { fetch_test_data(&client, range).await.unwrap() });

    let converter = CexQuotesConverter::new(block_times, symbols, quotes, best_cex_per_pair);
    let price_map = converter.convert_to_prices();

    let test_quotes = create_test_cex_quotes();

    let test_block = &price_map
        .iter()
        .find(|(block, _)| block == &18264795)
        .unwrap()
        .1;

    let cex_quotes = test_block
        .quotes
        .get(&CexExchange::Binance)
        .unwrap()
        .get(&Pair(WETH_ADDRESS, USDT_ADDRESS))
        .unwrap()
        .clone();

    assert_eq!(cex_quotes, test_quotes);
}

fn create_test_cex_quotes() -> Vec<CexQuote> {
    vec![
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271963002000,
            price:     (
                Rational::try_from_float_simplest(1648.31).unwrap(),
                Rational::try_from_float_simplest(1648.4).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.0256).unwrap(),
                Rational::try_from_float_simplest(0.4466).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271965002000,
            price:     (
                Rational::try_from_float_simplest(1652.87).unwrap(),
                Rational::try_from_float_simplest(1654.01).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.98).unwrap(),
                Rational::try_from_float_simplest(0.5564).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271975006000,
            price:     (
                Rational::try_from_float_simplest(1652.36).unwrap(),
                Rational::try_from_float_simplest(1652.52).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(7.755).unwrap(),
                Rational::try_from_float_simplest(4.5).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271993012000,
            price:     (
                Rational::try_from_float_simplest(1648.29).unwrap(),
                Rational::try_from_float_simplest(1649.25).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(15.0497).unwrap(),
                Rational::try_from_float_simplest(0.025).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696272023022000,
            price:     (
                Rational::try_from_float_simplest(1651.97).unwrap(),
                Rational::try_from_float_simplest(1651.99).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.5151).unwrap(),
                Rational::try_from_float_simplest(1.3979).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696272262921000,
            price:     (
                Rational::try_from_float_simplest(1654.46).unwrap(),
                Rational::try_from_float_simplest(1654.47).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.446).unwrap(),
                Rational::try_from_float_simplest(24.8339).unwrap(),
            ),
        },
    ]
}
