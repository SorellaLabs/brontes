# Metadata Tables

## AddressMeta Table

**Table Name:** `AddressMeta`

**Description:** Contains comprehensive metadata about blockchain addresses.

**Key:** Address

**Value:** [`AddressMetadata`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L15)

**Fields:**

- **entity_name**, **nametag**, **labels**, **address_type**: Basic identification and classification data about the address.
- **contract_info**: [`ContractInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L209) - Details about the contract if the address is a smart contract.
- **ens**: Optional ENS name associated with the address.
- **social_metadata**: [`Socials`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_metadata.rs#L234) - Links to social media and other external profiles related to the entity.

## Searcher Info Tables

---

**Table Names:** `SearcherEOAs` and `SearcherContracts`

**Description:** Stores metadata about Ethereum addresses (EOAs and Contracts) involved in MEV search activities.

**Key:** Address

**Value:** [`SearcherInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/searcher.rs#L21)

**Fields:**

- **fund**, **mev_count**, **pnl**, **gas_bids**: Key financial and operational metrics.
- **builder**: If the searcher is vertically integrated, the builder's address.
- **config_labels**: Types of MEV activities the searcher is involved in.
- **sibling_searchers**: Addresses of related searcher accounts.

## Builder Table

---

**Table Name:** `Builder`

**Description:** Contains information about Ethereum block builders, including their operational and financial metrics.

**Key:** Address (Coinbase transfer address)

**Value:** [`BuilderInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/builder.rs#L21)

**Fields:**

- **name**, **fund**, **pub_keys**: Basic identification and operational details.
- **searchers_eoas**, **searchers_contracts**: Lists of associated searcher addresses.
- **ultrasound_relay_collateral_address**: Address used for collateral in ultrasound relay transactions.
