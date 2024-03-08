# Brontes database

**Contextualizing the Chain:**

Brontes leverages a blend of off-chain data and on-chain metadata to enrich its analytical capabilities, featuring:

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
