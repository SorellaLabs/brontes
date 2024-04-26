# brontes run

Run brontes

```bash
$ brontes run --help
Usage: brontes run [OPTIONS]

Options:
  -s, --start-block <START_BLOCK>
          Optional Start Block, if omitted it will run at tip until killed

  -e, --end-block <END_BLOCK>
          Optional End Block, if omitted it will run historically & at tip until killed

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -m, --max-tasks <MAX_TASKS>
          Optional Max Tasks, if omitted it will default to 80% of the number of physical cores on your machine

      --min-batch-size <MIN_BATCH_SIZE>
          Optional minimum batch size
          
          [default: 500]

  -q, --quote-asset <QUOTE_ASSET>
          Optional quote asset, if omitted it will default to USDT
          
          [default: 0xdAC17F958D2ee523a2206206994597C13D831ec7]

  -i, --inspectors <INSPECTORS>
          Inspectors to run. If omitted it defaults to running all inspectors

      --price-tw-before <CEX_TIME_WINDOW_BEFORE>
          The sliding time window (BEFORE) for cex prices relative to the block timestamp
          
          [default: 0.5]

      --price-tw-after <CEX_TIME_WINDOW_AFTER>
          The sliding time window (AFTER) for cex prices relative to the block timestamp
          
          [default: 2.0]

  -c, --cex-exchanges <CEX_EXCHANGES>
          Centralized exchanges to consider for cex-dex inspector
          
          [default: Binance,Coinbase,Okex,BybitSpot,Kucoin]

  -f, --force-dex-pricing
          Ensures that dex prices are calculated at every block, even if the db already contains the price

      --force-no-dex-pricing
          Turns off dex pricing entirely, inspectors requiring dex pricing won't calculate USD pnl if we don't have dex pricing in the db & will only calculate token pnl

      --behind-tip <BEHIND_TIP>
          How many blocks behind chain tip to run
          
          [default: 3]

      --cli-only
          

      --init-crit-tables
          

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Display:
  -v, --verbosity...
          Set the minimum log level.
          
          -v      Errors
          -vv     Warnings
          -vvv    Info
          -vvvv   Debug
          -vvvvv  Traces (warning: very verbose!)

      --quiet
          Silence all log output
```