# Tables Schema

This page serves as a directory for the Brontes database tables. Click on the links provided for detailed schemas and further explanations about each table’s role and contents.

## Pricing Data

- **[`DexPrice`](./schema/pricing.md#dex-price-table-schema)**: DEX pricing data at a transaction level of granularity.
- **[`CexPrice`](./schema/pricing.md#cex-price-table-schema)**: Price data from centralized exchanges.
- **[`CexTrades`](./schema/pricing.md#cex-trades-table-schema)**: Trade data from centralized exchanges.

## Block Data

- **[`BlockInfo`](./schema/block.md#block-info-table-schema)**: P2P and mev-boost data for each block.
- **[`TxTraces`](./schema/block.md#tx-traces-table-schema)**: Transaction trace data for each block.

## Metadata

- **[`AddressMetadata`](./schema/metadata.md#addressmeta-table)**: Detailed address metadata.
- **[`Searcher`](./schema/metadata.md#searcher-info-tables)**: Sarcher eoas and contracts metadata.
- **[`Builder`](./schema/metadata.md#builder-table)**: Ethereum block builders.

## Classification Data

These tables are used during the classification process.

- **[`AddressToProtocolInfo`](./schema/classification.md#addresstoprotocolinfo-table)**: Maps addresses to specific protocols & pool tokens.
- **[`TokenDecimals`](./schema/classification.md#tokendecimals-table)**: Token decimals & symbols.

## Brontes Output Data

- [`MevBlocks`](./schema/mev_blocks.md#mevblocks-table): Output of Brontes’ analysis, containing the mev bundles identified in each block.

## Misc

- **[`PoolCreationBlocks`](./schema/misc.md#poolcreationblocks-table)**: Tracks the creation of liquidity pools, which informs the dex pricing module on what pools to initialize for a given block range.
- **[`InitializedState`](./schema/misc.md#initializedstate-table)**: Indicates the state loaded into Brontes to identify the data that needs to be downloaded from Clickhouse.
