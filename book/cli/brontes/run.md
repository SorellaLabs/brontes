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

  -b, --tw-before <TIME_WINDOW_BEFORE>
          The sliding time window (BEFORE) for cex prices or trades relative to the block timestamp
          
          [default: 10]

  -a, --tw-after <TIME_WINDOW_AFTER>
          The sliding time window (AFTER) for cex prices or trades relative to the block timestamp
          
          [default: 20]

      --op-tw-before <TIME_WINDOW_BEFORE_OPTIMISTIC>
          The time window (BEFORE) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 5.0]

      --op-tw-after <TIME_WINDOW_AFTER_OPTIMISTIC>
          The time window (AFTER) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 10.0]

      --mk-time <QUOTES_PRICE_TIME>
          Cex Dex Quotes price time
          
          [default: 0.0]

  -c, --cex-exchanges <CEX_EXCHANGES>
          CEX exchanges to consider for cex-dex analysis
          
          [default: Binance,Coinbase,Okex,BybitSpot,Kucoin]

  -f, --force-dex-pricing
          Force DEX price calculation for every block, ignoring existing database values

      --force-no-dex-pricing
          Disables DEX pricing. Inspectors needing DEX prices will only calculate token PnL, not USD PnL, if DEX pricing is unavailable in the database

      --behind-tip <BEHIND_TIP>
          Number of blocks to lag behind the chain tip when processing
          
          [default: 10]

      --cli-only
          Legacy, run in CLI only mode (no TUI) - will output progress bars to stdout

      --with-metrics
          Export metrics

      --enable-fallback
          Wether or not to use a fallback server

      --fallback-server <FALLBACK_SERVER>
          Address of the fallback server. Triggers database writes if the main connection fails, preventing data loss

  -r, --run-id <RUN_ID>
          Set a custom run ID used when inserting data into the Clickhouse
          
          If omitted, the ID will be automatically incremented from the last run stored in the Clickhouse database.

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