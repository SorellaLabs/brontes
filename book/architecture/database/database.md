# Brontes Database

Brontes uses a local libmdbx database to store off-chain data for its analysis pipeline. The data comes from a Clickhouse database managed by Sorella Labs. It includes centralize exchange quotes and trade data, mempool and relay data, address metadata, and more. For details on the specific tables and their schemas, see the [Tables Schema](./tables_schema.md) page.

## Database Sync

On startup, Brontes syncs its local database by downloading the needed data from Clickhouse.

<div style="text-align: center;">
    <img src="./diagrams/db-download.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
    <p style="font-style: italic;">Figure 1: Data download from Clickhouse to Brontes local storage</p>
</div>

### Snapshot Sync

To manage cloud egress costs, we don't currently provide api access to our clickhouse database for historical sync. Instead, users must download the latest db snapshot made available every Monday and Thursday. See the [Installation Guide](../../installation/installation.md) for detailed instructions.

<div style="text-align: center;">
    <img src="./diagrams/user-download-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 500px; height: auto;">
    <p style="font-style: italic;">Figure 2: User db snapshot download process.</p>
</div>

## Data Flow

Brontes adapts its data retrieval method based on its operational mode: for historical block analysis, it accesses the pre-stored data locally from its libmdbx database; when operating at chain tip, it retrieves data through the Brontes API.

<div style="text-align: center;">
    <img src="./diagrams/data-query-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 500px; height: auto;">
    <p style="font-style: italic;">Figure 3: Querying methods for historical blocks and chain tip.</p>
</div>

> Note
> Users that want to run brontes at chain tip, must request API access to query the data at chain tip. Configuration details for API access can be found in the [Installation Guide](../../installation/installation.md).

### Data and Usage

The data stored by Brontes can be categorized into three main types:

#### 1. Block-Specific Data

Each value is mapped to a specific block.

The `Metadata` struct aggregates the essential block specific data, used by all [`Inspectors`](https://sorellalabs.github.io/brontes/docs/brontes_inspect/index.html) during their analysis.

```rust,ignore
pub struct Metadata {
    pub block_metadata: BlockMetadata,
    pub cex_quotes:     CexPriceMap,
    pub dex_quotes:     Option<DexQuotes>,
    pub builder_info:   Option<BuilderInfo>,
    pub cex_trades:     Option<Arc<Mutex<CexTradeMap>>>,
}
```

- **[`BlockInfo`](./schema/block.md#block-info-table)**: P2P transaction and block data and mev-boost data.
- **[`DexPrice`](./schema/pricing.md#dex-price-table)**: DEX pricing with transaction level granularity for all active tokens in the block.
- **[`CexPrice`](./schema/pricing.md#cexprice-table)** and **[`CexTrades`](./schema/pricing.md#cex-trades-table-schema)**: Centralized exchange quotes and trade data.
- **[`BuilderInfo`](./schema/metadata.md#builder-table)**: Information on the block builder.

#### 2. Range-Agnostic Data

Valid across the full block range. This primarily includes:

**Data for Decoding & Normalization**:

- [`TokenDecimals`](./schema/classification.md#token-decimals-table): Holds token decimal precision, which is used to normalize token amounts to a malachite rational representation.
- [`AddressToProtocolInfo`](./schema/classification.md#addresstoprotocolinfo-table): Maps blockchain addresses to protocol-specific information, facilitating transaction decoding and normalization.

**Metadata used by the Inspectors**:

This data is primarily used by the inspectors for filtering and analysis purposes. It is queried on an ad-hoc basis via the database handle provided by the inspectors' `SharedInspectorsUtils`.

- [`BuilderInfo`](./schema/metadata.md#builder-table): Information on ethereum block builders, including aggregate pnl & block count.
- [`SearcherInfo`](./schema/metadata.md#searcher-info-tables): Information on searchers eoas and contracts with fund information, associated searchers, vertically integrated builders, and summary statistics on mev bundle count and pnl by mev type.
- [`AddressMetadata`](./schema/metadata.md#address-metadata-table): Contains detailed metadata for addresses.

#### 3. Analysis Output Data

Stores the output of the analysis pipeline in the [`MevBlocks`](./schema/mev_blocks.md#mev-blocks-table-schema) table.
