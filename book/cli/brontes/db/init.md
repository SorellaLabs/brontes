# brontes db init

For a given range, will fetch all data from the api and insert it into libmdbx

```bash
$ brontes db init --help
Usage: brontes db init [OPTIONS]

Options:
  -i, --init-libmdbx
          Initialize the local Libmdbx DB

  -t, --tables-to-init <TABLES_TO_INIT>
          Libmdbx tables to init: TokenDecimals AddressToTokens AddressToProtocol CexPrice Metadata PoolState DexPrice CexTrades

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --price-tw-before <CEX_TIME_WINDOW_BEFORE>
          The sliding time window (BEFORE) for cex prices relative to the block timestamp
          
          [default: 0.5]

      --price-tw-after <CEX_TIME_WINDOW_AFTER>
          The sliding time window (AFTER) for cex prices relative to the block timestamp
          
          [default: 1.0]

  -c, --cex-exchanges <CEX_EXCHANGES>
          Centralized exchanges to consider for cex-dex inspector
          
          [default: Binance,Coinbase,Okex,BybitSpot,Kucoin]

  -s, --start-block <START_BLOCK>
          Start Block to download metadata from Sorella's MEV DB

  -e, --end-block <END_BLOCK>
          End Block to download metadata from Sorella's MEV DB

  -d, --download-dex-pricing
          Download Dex Prices from Sorella's MEV DB for the given block range. If false it will run the dex pricing locally using raw on-chain data

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