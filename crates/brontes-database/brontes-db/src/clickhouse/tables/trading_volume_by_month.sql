CREATE TABLE cex.trading_volume_by_month (
    `month`       Date,
    `symbol`      LowCardinality(String),
    `exchange`    LowCardinality(String),
    `sum_volume`  Float64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(month)
ORDER BY (month, symbol, exchange);

CREATE MATERIALIZED VIEW cex.mv_trading_volume_by_month
  TO cex.trading_volume_by_month
AS
SELECT
  -- convert your microsecond ts into the first day of its month
  toDate( toStartOfMonth( toDateTime(timestamp/1000000) ) ) AS month,
  symbol,
  exchange,
  amount       AS sum_volume
FROM cex.normalized_trades;