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
          

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --clear-cex-flags
          

      --clear-tx-traces-flags
          

      --clear-dex-pricing-flags
          

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