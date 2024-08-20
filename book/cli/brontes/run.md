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

      --ranges <RANGES>...
          Optional Multiple Ranges, format: "start1-end1 start2-end2 ..." Use this if you want to specify the exact, non continuous block ranges you want to run

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

      --initial-pre <INITIAL_VWAP_PRE>
          The initial sliding time window (BEFORE) for cex prices or trades relative to the block timestamp
          
          [default: 0.05]

      --initial-post <INITIAL_VWAP_POST>
          The initial sliding time window (AFTER) for cex prices or trades relative to the block timestamp
          
          [default: 0.05]

  -b, --max-vwap-pre <MAX_VWAP_PRE>
          The maximum sliding time window (BEFORE) for cex prices or trades relative to the block timestamp
          
          [default: 10.0]

  -a, --max-vwap-post <MAX_VWAP_POST>
          The maximum sliding time window (AFTER) for cex prices or trades relative to the block timestamp
          
          [default: 20.0]

      --vwap-scaling-diff <VWAP_SCALING_DIFF>
          Defines how much to extend the post-block time window before the pre-block
          
          [default: 0.3]

      --vwap-time-step <VWAP_TIME_STEP>
          Size of each extension to the vwap calculations time window
          
          [default: 0.01]

      --weights-vwap
          Use block time weights to favour prices closer to the block time

      --weights-pre-vwap <PRE_DECAY_WEIGHT_VWAP>
          Rate of decay of bi-exponential decay function see calculate_weight in brontes_types::db::cex
          
          [default: -0.0000005]

      --weights-post-vwap <POST_DECAY_WEIGHT_VWAP>
          Rate of decay of bi-exponential decay function see calculate_weight in brontes_types::db::ce
          
          [default: -0.0000002]

      --initial-op-pre <INITIAL_OPTIMISTIC_PRE>
          The initial time window (BEFORE) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 0.05]

      --initial-op-post <INITIAL_OPTIMISTIC_POST>
          The initial time window (AFTER) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 0.3]

      --max-op-pre <MAX_OPTIMISTIC_PRE>
          The maximum time window (BEFORE) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 5.0]

      --max-op-post <MAX_OPTIMISTIC_POST>
          The maximum time window (AFTER) for cex prices or trades relative to the block timestamp for fully optimistic calculations
          
          [default: 10.0]

      --optimistic-scaling-diff <OPTIMISTIC_SCALING_DIFF>
          Defines how much to extend the post-block time window before the pre-block
          
          [default: 0.2]

      --optimistic-time-step <OPTIMISTIC_TIME_STEP>
          Size of each extension to the optimistic calculations time window
          
          [default: 0.1]

      --weights-op
          Use block time weights to favour prices closer to the block time

      --weights-pre-op <PRE_DECAY_WEIGHT_OPTIMISTIC>
          Rate of decay of bi-exponential decay function see calculate_weight in brontes_types::db::cex
          
          [default: -0.0000003]

      --weights-post-op <POST_DECAY_WEIGHT_OPTIMISTIC>
          Rate of decay of bi-exponential decay function see calculate_weight in brontes_types::db::ce
          
          [default: -0.00000012]

      --quote-offset <QUOTE_OFFSET>
          Cex Dex Quotes price time offset from block timestamp
          
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

  -w, --waterfall
          shows a cool display at startup

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