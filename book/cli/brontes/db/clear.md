# brontes db clear

Clear a libmdbx table

```bash
$ brontes db clear --help
Usage: brontes db clear [OPTIONS]

Options:
  -t, --tables <TABLES>
          Tables to clear
          
          [default: CexPrice,DexPrice,CexTrades,BlockInfo,InitializedState,MevBlocks,TokenDecimals,AddressToProtocolInfo,PoolCreationBlocks,Builder,AddressMeta,SearcherEOAs,SearcherContracts,SubGraphs,TxTraces]

      --clear-metadata-flags
          Mark metadata as uninitialized in the initialized state table

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --clear-cex-quotes-flags
          Mark cex quotes as uninitialized in the initialized state table

      --clear-cex-trades-flags
          Mark cex trades as uninitialized in the initialized state table

      --clear-tx-traces-flags
          Mark tx traces as uninitialized in the initialized state table

      --clear-dex-pricing-flags
          Mark dex pricing as uninitialized in the initialized state table

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