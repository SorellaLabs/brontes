# Metadata Tables

## Address Metadata Table Schema

**Table Name:** `AddressMetadata`

**Description:** This table stores comprehensive metadata about blockchain addresses, leveraging a key-value structure where each `Address` key links to detailed `AddressMetadata`.

**Key:** `Address`

- **Type:** `Address`
- **Description:** The blockchain address that uniquely identifies each entry in the table.

**Value:** [`AddressMetadata`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/address_metadata.rs#L15)

**Description:** Contains metadata including entity names, tags, and extensive contract details associated with the blockchain address.

**Fields**:

- **entity_name**:

  - **Type**: `Option<String>`
  - **Description**: Official or widely recognized name of the entity associated with the address.

- **nametag**:

  - **Type**: `Option<String>`
  - **Description**: An alternative or abbreviated name used to identify the entity.

- **labels**:

  - **Type**: `Vec<String>`
  - **Description**: Descriptive labels that categorize the address by its roles or characteristics within the blockchain ecosystem.

- **address_type**:

  - **Type**: `Option<String>`
  - **Description**: Classifies the type of address (e.g., Contract, EOA) and its functional role.

- **contract_info** ([`Option<ContractInfo>`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/address_metadata.rs#L210)):

  - **Description**: Specific details about the contract if the address is a smart contract.
    - **verified_contract**:
      - **Type**: `Option<bool>`
      - **Description**: Indicates whether the contract is verified, suggesting transparency and security.
    - **contract_creator**:
      - **Type**: `Option<Address>`
      - **Description**: The address that deployed the contract, providing insights into its origins.
    - **reputation**:
      - **Type**: `Option<u8>`
      - **Description**: A reputation score based on historical activities and community trust.

- **ens**:

  - **Type**: `Option<String>`
  - **Description**: Associated Ethereum Name Service (ENS) domain that resolves to this address.

- **social_metadata** ([`Socials`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/address_metadata.rs#L235)):
  - **Description**: Links to social media and other external profiles related to the entity.
    - **twitter**:
      - **Type**: `Option<String>`
      - **Description**: Twitter handle associated with the address.
    - **twitter_followers**:
      - **Type**: `Option<u64>`
      - **Description**: Number of followers on Twitter, indicating the social reach or influence.
    - **website_url**:
      - **Type**: `Option<String>`
      - **Description**: URL of the official website.
    - **crunchbase**:
      - **Type**: `Option<String>`
      - **Description**: Crunchbase profile link, providing business insights.
    - **linkedin**:
      - **Type**: `Option<String>`
      - **Description**: LinkedIn profile link for professional networking.

## Searcher Table Schema

The Brontes database includes two key-value tables, `SearcherEOAs` and `SearcherContracts`, that store metadata about Ethereum addresses associated with MEV searchers. Both tables use blockchain addresses as keys and share the same value structure defined by the `SearcherInfo` struct.

### Table Names

- **SearcherEOAs**: Stores information about externally owned accounts (EOAs) used by searchers.
- **SearcherContracts**: Stores information about smart contracts used by searchers.

**Key:** `Address`

- **Type:** `Address`
- **Description:** The blockchain address that uniquely identifies each entry in the table.

**Value:** [`SearcherInfo`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/searcher.rs#L21)

**Description**: Holds detailed information about MEV searchers, including their associated funds, MEV metrics, and potential vertical integrations with builders.

**Fields**:

- **fund**:
  - **Type**: [`Fund`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/searcher.rs#L268)
  - **Description**: The fund or entity backing the searcher, if any.
- **mev_count**:
  - **Type**: [`MevCount`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/mev/block.rs#L147)
  - **Description**: Counts of different types of MEV scenarios the searcher has participated in.
- **pnl**:
  - **Type**: [`TollByType`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/searcher.rs#L177)
  - **Description**: Profit and loss metrics for the searcher, categorized by MEV type.
- **gas_bids**:
  - **Type**: [`TollByType`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/db/searcher.rs#L177)
  - **Description**: Records of gas bids associated with the searcher's transactions, also categorized by MEV type.
- **builder**:
  - **Type**: `Option<Address>`
  - **Description**: If the searcher is vertically integrated, this field contains the associated builder’s address.
- **config_labels**:
  - **Type**: [`Vec<MevType>`](https://github.com/SorellaLabs/brontes/blob/8bccd73d327c14a775025ee7e355eace02da51a5/crates/brontes-types/src/mev/bundle/mod.rs#L86)
  - **Description**: Labels indicating the types of MEV the searcher is known for.
- **sibling_searchers**:
  - **Type**: `Vec<Address>`
  - **Description**: Addresses of other searchers that are considered 'siblings' or related entities, likely under the same operational control.

### Builder Info Table Schema

**Table Name:** `Builder`

**Description:** This table stores detailed information about builders, who are entities responsible for constructing and submitting transactions on the blockchain. Each entry is accessed using a blockchain `Address` as the key, which links to `BuilderInfo`.

**Key:** `Address`

- **Type:** `Address` (Coinbase Transfer Address)
- **Description:** The blockchain address that uniquely identifies a builder in the table.

**Value:** `BuilderInfo`

**Structure Definition:** Detailed information about the builder, including associated funds, public keys, and linked searchers.

**Fields**:

- **name**:

  - **Type**: `Option<String>`
  - **Description**: The name of the builder, if available.

- **fund**:

  - **Type**: `Option<Fund>`
  - **Description**: The fund or financial entity backing the builder, providing additional context about the builder’s financial resources and affiliations.

- **pub_keys**:

  - **Type**: `Vec<BlsPublicKey>`
  - **Description**: A list of BLS public keys used by the builder for cryptographic operations, crucial for verifying the builder’s transactions.

- **searchers_eoas**:

  - **Type**: `Vec<Address>`
  - **Description**: A list of addresses for externally owned accounts (EOAs) that are associated with the builder, typically used for searching and submitting profitable transactions.

- **searchers_contracts**:

  - **Type**: `Vec<Address>`
  - **Description**: A list of smart contract addresses associated with the builder, often involved in automated transaction submission or complex strategies.

- **ultrasound_relay_collateral_address**:
  - **Type**: `Option<Address>`
  - **Description**: If applicable, the address used for holding collateral in ultrasound relay transactions, providing a link to the financial stakes the builder maintains in network operations.
