# Brontes Database

The Brontes database stores essential off-chain data and on-chain metadata in a local libmdbx db. This data is collected and stored in a Clickhouse database, which is managed and hosted by Sorella Labs.

## Initializing the Database

Upon startup, Brontes will download the data from Clickhouse and store it in its local database.

<div style="text-align: center;">
    <img src="./diagrams/db-download.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
    <p style="font-style: italic;">Figure 1: Production data flow from Clickhouse to Brontes' local storage</p>
</div>

### Snapshot Sync

To manage cloud egress costs, we do not currently provide api access to our clickhouse database for historical sync. Instead, users must download the latest db snapshot made available every Monday and Thursday. See the [Installation Guide](./installation/installation.md) for detailed instructions.

<div style="text-align: center;">
    <img src="./diagrams/user-download-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 500px; height: auto;">
    <p style="font-style: italic;">Figure 2: User snapshot download and extraction process.</p>
</div>

### Live Sync

Users that want to run brontes at chain tip, must request API access so that they can query the data at chain tip. Configuration details for API access can be found in the [Installation Guide](./installation/installation.md).

## Data Flow

<div style="text-align: center;">
    <img src="./diagrams/data-query-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
    <p style="font-style: italic;">Figure 3: Querying methods for historical and real-time data..</p>
</div>

- **Historical Data**: Users can query historical data directly from their local libmdbx database, which is regularly updated via snapshots.
- **Real-Time Data**: For up-to-the-minute data, users must connect to the Brontes API, which accesses the latest data directly from the Clickhouse database.

## Data Flow

- metadata query for each block
- peripheral data queried ad hoc

Data Usage:

- Metadata struct: Data fetched for each block, baseline required data for the inspectors
- Peripheral data: Additional metadata queried by the inspectors to enrich their analysis and enable complex filtering

**Contextualizing the Chain:**

, featuring:

- **Pricing Data:**
  - DEX pricing with transaction-level granularity.
  - CEX trades and quotes for all major crypto exchanges.
- **Address Metadata:** Address labels for entities, funds, protocols, and extensive contract metadata.
- **P2P Data:** Timestamped mempool and block propagation data, courtesy of [Chainbound](https://www.chainbound.io/).
- **Searcher Metadata:**
  - Associated fund
  - MEV types engaged in
  - Vertically integrated builder (if applicable)
- **Builder Metadata:**
  - Name
  - Associated fund (if applicable)
  - BLS public keys
  - Vertically integrated searcher EOAs and contracts (if applicable)
  - Ultrasound relay collateral address (if applicable)
- **Relay Bid Data:** Block auction bid data from major relays since the Merge.

## Metadata

- Dex Pricing
- Cex Pricing
  - CexPriceMap
  - CexTradeMap
- Block Metadata
- Builder Info

## Peripheral Data

**Core Data:**

- TokenDecimals
- AddressToProtocolInfo

**Metadata:**

- BuilderInfo
- SearcherInfo
  - SearcherEOAs
  - SearcherContracts
- Address Metadata

**Mev Data:**

- Mev Blocks

**Miscellaneous:**

- PoolCreationBlocks
- InitializedState
