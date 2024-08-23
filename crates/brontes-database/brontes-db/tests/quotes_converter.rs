use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use brontes_types::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::cex::{
        quotes::{CexQuote, CexQuotesConverter},
        CexExchange,
    },
    init_thread_pools,
    pair::Pair,
};
use malachite::Rational;
mod shared;
use shared::fetch_test_data;

#[brontes_macros::test]
async fn test_cex_quote_conversion() {
    init_thread_pools(32);
    let client = Clickhouse::new_default(None).await;
    let range = CexRangeOrArbitrary::Range(18264694, 18264795);
    let (block_times, symbols, quotes, best_cex_per_pair) =
        fetch_test_data(&client, range).await.unwrap();

    let converter = CexQuotesConverter::new(block_times, symbols, quotes, best_cex_per_pair);
    let price_map = converter.convert_to_prices();

    let test_quotes = create_test_cex_quotes();

    let test_block = &price_map
        .iter()
        .find(|(block, _)| block == &18264694)
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

    let expected_length = 18264795 - 18264694;
    assert_eq!(price_map.len(), expected_length)
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
            timestamp: 1696271964002000,
            price:     (
                Rational::try_from_float_simplest(1649.74).unwrap(),
                Rational::try_from_float_simplest(1666.1).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.3615).unwrap(),
                Rational::try_from_float_simplest(8.378).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271974005000,
            price:     (
                Rational::try_from_float_simplest(1653.27).unwrap(),
                Rational::try_from_float_simplest(1653.85).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(17.4219).unwrap(),
                Rational::try_from_float_simplest(2.0796).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696271992011000,
            price:     (
                Rational::try_from_float_simplest(1648.0).unwrap(),
                Rational::try_from_float_simplest(1648.15).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.8695).unwrap(),
                Rational::try_from_float_simplest(0.1945).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696272022022000,
            price:     (
                Rational::try_from_float_simplest(1651.69).unwrap(),
                Rational::try_from_float_simplest(1651.7).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(4.6408).unwrap(),
                Rational::try_from_float_simplest(0.7662).unwrap(),
            ),
        },
        CexQuote {
            exchange:  CexExchange::Binance,
            timestamp: 1696272262121000,
            price:     (
                Rational::try_from_float_simplest(1654.55).unwrap(),
                Rational::try_from_float_simplest(1654.56).unwrap(),
            ),
            amount:    (
                Rational::try_from_float_simplest(0.0268).unwrap(),
                Rational::try_from_float_simplest(41.2785).unwrap(),
            ),
        },
    ]
}
