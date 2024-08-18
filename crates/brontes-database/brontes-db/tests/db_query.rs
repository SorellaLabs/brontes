use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use futures::{stream::FuturesUnordered, StreamExt};

mod shared;
use shared::fetch_test_data;

#[brontes_macros::test]
async fn test_query_retry() {
    let client = Clickhouse::new_default(None).await;

    let range = CexRangeOrArbitrary::Range(19000000, 19001000);

    let mut futs = FuturesUnordered::new();

    for _ in 0..30 {
        futs.push(fetch_test_data(&client, range));
    }

    while let Some(result) = futs.next().await {
        assert!(result.is_ok());
    }
}
