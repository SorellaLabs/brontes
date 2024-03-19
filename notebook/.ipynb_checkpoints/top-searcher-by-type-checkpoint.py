import polars as pl

def read_data(path_to_blocks, path_to_bundles):
    blocks_df = pl.read_parquet(path_to_blocks)
    bundles_df = pl.read_parquet(path_to_bundles)
    return blocks_df, bundles_df

def filter_mev_types(bundles_df):
    # Exclude rows where mev_type is 'SearcherTx' or 'Unknown'
    filtered_df = bundles_df.filter(~pl.col("mev_type").is_in(["SearcherTx", "Unknown"]))
    return filtered_df

def get_top_searchers_by_pnl(bundles_df):
    # Group by mev_contract, aggregate profit_usd, and sort by total_profit_usd descending
    top_searchers = bundles_df.groupby("mev_contract").agg([
        pl.col("profit_usd").sum().alias("total_profit_usd")
    ]).sort("total_profit_usd", reverse=True).head(30)
    return top_searchers

if __name__ == "__main__":
    path_to_blocks = "../db/parquet/block_table.parquet"
    path_to_bundles = "../db/parquet/bundle_table.parquet"

    blocks_df, bundles_df = read_data(path_to_blocks, path_to_bundles)

    # Filter bundles_df for relevant MEV types
    filtered_bundles_df = filter_mev_types(bundles_df)

    # Get top 30 searchers by PNL
    top_searchers_by_pnl = get_top_searchers_by_pnl(filtered_bundles_df)

    print("Top 30 Searchers by PNL (excluding SearcherTx and Unknown MEV types):")
    print(top_searchers_by_pnl)
