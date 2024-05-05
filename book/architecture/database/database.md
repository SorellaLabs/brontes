# Brontes Database

The Brontes database serves as a critical component of our platform, synthesizing off-chain data to support robust blockchain analytics. This document elucidates the data collection, storage, and access mechanisms implemented in Brontes.

## Data Collection and Storage

Upon initialization, Brontes engages with the Clickhouse database to download essential data for predefined historical ranges. This data is subsequently stored in a local libmdbx database, enhancing Brontes' performance by mitigating its reliance on continuous central database connectivity.

**Diagram 1: Brontes Initial Data Flow**

<div style="text-align: center;">
 <img src="./diagrams/db-download.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

_Figure 1: Production data flow from Clickhouse to Brontes' local storage._

## Enhancing User Access Through Snapshots

To manage cloud egress costs effectively, Brontes provides bi-weekly snapshots of the libmdbx database. These snapshots are made available every Monday and Thursday, allowing users to maintain an up-to-date local database without incurring significant costs.

- **Snapshots**: Brontes provides regularly updated snapshots of the libmdbx database for users focused on historical analysis. These snapshots are refreshed twice a week, every Monday and Thursday. Users must download these snapshots to run Brontes. See the [Installation Guide](./installation/installation.md) for detailed instructions.

- **Real-Time Data**: Users that want to run brontes at chain tip, must request API access to connect directly to the Clickhouse database. This API provides a live stream of the data updates. Configuration details for API access can be found in the [Installation Guide](./installation/installation.md).

**Diagram 2: Snapshot Management in Brontes**

<div style="text-align: center;">
 <img src="./diagrams/user-download-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

_Figure 2: Showcases the snapshot download and extraction process._

For detailed setup and operational instructions, refer to our [Installation Guide](./installation/installation.md).

## Querying Data

Brontes employs a dual querying approach to meet different user needs:

- **Historical Data**: Users can query historical data directly from their local libmdbx database, which is regularly updated via snapshots. This facilitates comprehensive data analysis without the need for real-time internet connectivity.
- **Real-Time Data**: For up-to-the-minute data, users must connect to the Brontes API, which accesses the latest data directly from the Clickhouse database. This is crucial for users requiring immediate data for real-time decision-making.

**Diagram 3: Querying Data in Brontes**

<div style="text-align: center;">
 <img src="./diagrams/data-query-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

_Figure 3: Details the querying methods for historical and real-time data._

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
