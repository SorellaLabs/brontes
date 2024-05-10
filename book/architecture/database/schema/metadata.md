# Metadata Tables

## AddressMeta Table

---

**Table Name:** `AddressMeta`

**Description:** Comprehensive address metadata.

**Key:** Address

**Value:** [`AddressMetadata`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L15)

**Fields:**

- **entity_name**, **nametag**: Entity name and alias.
- **labels**: List of address labels.
- **address_type**: Type of address (DEX, CEX, Aggregator...).
- **contract_info**: [`ContractInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L209) - Details about the contract if the address is a smart contract.
- **ens**: Optional ENS name associated with the address.
- **social_metadata**: [`Socials`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L234) - Links to social media and other external profiles related to the entity.

## Searcher Info Tables

---

**Table Names:** `SearcherEOAs` and `SearcherContracts`

**Description:** Searcher EOA & Contract Metadata.

**Key:** Address

**Value:** [`SearcherInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/searcher.rs#L21)

**Fields:**

- **fund**: Fund the searcher address is associated with.
- **mev_count**: [`TollByType`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/mev/block.rs#L147) - MEV bundle count by type.
- **pnl**: [`TollByType`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/searcher.rs#L21) - Aggregate Pnl by MEV type.
- **gas_bids**: [`TollByType`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/searcher.rs#L21) - Gas bids by MEV type.
- **builder**: If the searcher is vertically integrated, the builder's address.
- **config_labels**: Types of MEV this searcher captures. This is set at the config level in `config/searcher_config.toml`.
- **sibling_searchers**: Addresses of searcher accounts associated with this address. This is needed so that we can accurately calculate PnL when searchers send their profit to a bank address or on of their other searcher addresses.

## Builder Table

---

**Table Name:** `Builder`

**Description:** Contains information about Ethereum block builders.

**Key:** Address (Coinbase transfer address)

**Value:** [`BuilderInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/builder.rs#L21)

**Fields:**

- **name**, **fund**, **pub_keys**: Basic identification and operational details.
- **searchers_eoas**, **searchers_contracts**: Lists of the builder's searcher addresses.
- **ultrasound_relay_collateral_address**: Address used to deposit collateral for the optimistic ultrasound relay.
