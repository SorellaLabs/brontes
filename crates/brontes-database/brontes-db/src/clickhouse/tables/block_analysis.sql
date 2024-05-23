CREATE TABLE brontes.block_analysis ON CLUSTER eth_cluster0 
(
    `block_number` UInt64,

    `total_mev_profit` Float64,
    `all_top_searcher` String,
    `all_top_fund` String,
    `all_average_profit` Float64,
    `all_unique_searchers` UInt64,
    `all_unique_funds` UInt64,

    `most_arbed_pair` Tuple(String, String),
    `most_arbed_pool` String,
    `most_arbed_dex` String,
    `arb_total_revenue` Float64,
    `arb_total_profit` Float64,
    `arb_top_searcher` String,
    `arb_top_fund` String,
    `arb_unique_searchers` UInt64,
    `arb_unique_funds` UInt64,
    
    `most_sandwiched_pair` Tuple(String, String),
    `most_sandwiched_pool` String,
    `most_sandwiched_dex` String,
    `sandwich_total_revenue` Float64,
    `sandwich_total_profit` Float64,
    `sandwich_total_swapper_loss` Float64,
    `sandwich_top_searcher` String,
    `sandwich_unique_searchers` UInt64,

    `most_jit_pair` Tuple(String, String),
    `most_jit_pool` String,
    `most_jit_dex` String,
    `jit_total_revenue` Float64,
    `jit_total_profit` Float64,
    `jit_top_searcher` String,
    `jit_unique_searchers` UInt64,

    `most_jit_sandwiched_pair` Tuple(String, String),
    `most_jit_sandwiched_pool` String,
    `most_jit_sandwiched_dex` String,
    `jit_sandwich_total_revenue` Float64,
    `jit_sandwich_total_profit` Float64,
    `jit_sandwich_total_swapper_loss` Float64,
    `jit_sandwich_top_searcher` String,
    `jit_sandwich_unique_searchers` UInt64,

    `cex_dex_most_arb_pair_rev` Tuple(String, String),
    `cex_dex_most_arb_pool_rev`   String,
    `cex_dex_most_arb_pair_profit` Tuple(String, String),
    `cex_dex_most_arb_pool_profit` String,
    `cex_dex_total_rev`            Float64,
    `cex_dex_total_profit`         Float64,
    `cex_top_searcher`             String,
    `cex_top_fund`                 String,

    `most_liquidated_token` String,
    `most_liquidated_protocol` String,
    `liquidation_total_revenue`            Float64,
    `liquidation_total_profit`            Float64,
    `liquidation_average_profit_margin`    Float64,
    `liquidation_top_searcher`             String,
    `liquidation_unique_searchers`         UInt64,
    `total_usd_liquidated`     Float64,

    `last_updated` UInt64 DEFAULT now()
)

ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/block_analysis', '{replica}', `last_updated`)
PRIMARY KEY (`block_number`)
ORDER BY (`block_number`)
