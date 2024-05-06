# Tables Schema

This page serves as a directory for the various tables used in the Brontes database. Each table is categorized by its function within the system. Click on the links provided for detailed schemas and further explanations about each table’s role and contents.

## Pricing Data

- **[`DexPrice`](./schema/pricing.md#dex-price-table-schema)**: Provides decentralized exchange pricing data at a transaction level of granularity.
- **[`CexPrice`](./schema/pricing.md#cex-price-table-schema)**: Contains price data from centralized exchanges.
- **[`CexTrades`](./schema/pricing.md#cex-trades-table-schema)**: Holds trade data from centralized exchanges.

## Block Data

- **[`BlockInfo`](./schema/block.md#block-info-table-schema)**: Stores p2p and mev-boost data for each block.
- **[`TxTraces`](./schema/block.md#tx-traces-table-schema)**: Contains transaction trace data for each block.

## Metadata

- **[`AddressMetadata`](./schema/metadata.md#addressmeta-table)**: Contains detailed metadata for blockchain addresses.
- **[`Searcher`](./schema/metadata.md#searcher-info-tables)**: Holds metadata on searcher eoas and contracts.
- **[`Builder`](./schema/metadata.md#builder-table)**: Holds metadata on ethereum block builders.

## Classification Data

These tables include data used during the classification process.

- **[`AddressToProtocolInfo`](./schema/classification.md#addresstoprotocolinfo-table)**: Maps addresses to specific protocols & the tokens for th
- **[`TokenDecimals`](./schema/classification.md#tokendecimals-table)**: Provides decimal precision for various tokens, crucial for financial calculations.

## Brontes Output Data

This table are outputs from Brontes’ analysis, containing the mev bundles identified in each block.

- [`MevBlocks`](./schema/mev_blocks.md#mevblocks-table)

## Misc

- **[`PoolCreationBlocks`](./schema/misc.md#poolcreationblocks-table)**: Tracks the creation of liquidity pools, which informs the dex pricing module on what pools to initialize for a given block range.
- **[`InitializedState`](./schema/misc.md#initializedstate-table)**: Indicates the state loaded into Brontes to identify the data that needs to be downloaded from Clickhouse.
