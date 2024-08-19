use std::time::Duration;

use brontes_database::{clickhouse::Clickhouse, libmdbx::cex_utils::CexRangeOrArbitrary};
use futures::{stream::FuturesUnordered, StreamExt};

mod shared;
use shared::fetch_test_data;
use tokio::time::sleep;

#[brontes_macros::test]
async fn test_query_retry() {
    let client = Clickhouse::new_default(None).await;

    let range = CexRangeOrArbitrary::Range(19000000, 19001000);

    let mut futs = FuturesUnordered::new();

    for _ in 0..10 {
        futs.push(fetch_test_data(&client, range));
        sleep(Duration::from_millis(550)).await;
    }

    while let Some(result) = futs.next().await {
        assert!(result.is_ok());
    }
}
