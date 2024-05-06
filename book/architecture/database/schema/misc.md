# Miscellaneous Table

## PoolsCreationBlock Table

**Table Name:** `PoolsCreationBlock`

**Description:** Tracks the creation of liquidity pools within specific blocks, essential for the dex pricing module which uses this information to initialize and consider which pools to load for state management during a given block range.

**Key:** Block number (`u64`)

- **Type:** `u64`
- **Description:** The block number in which liquidity pools were created.

**Value:** [`PoolsToAddresses`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/pool_creation_block.rs#L11)

- **Type:** `Vec<Address>`
- **Description:** A list of addresses representing newly created liquidity pools within the specified block.

### Field Details

- **Addresses**:
  - **Type:** `Vec<Address>`
  - **Description:** Contains the blockchain addresses of the liquidity pools created in the corresponding block, facilitating targeted analysis and interactions with these new pools.

## InitializedState Table

---

**Table Name:** `InitializedState`

**Description:** Indicates which state data has been initialized and loaded into Brontes. This table helps in identifying the data that needs to be downloaded from Clickhouse to ensure that Brontes is up-to-date with the required data set.

**Key:** Block number (`u64`)

- **Type:** `u64`
- **Description:** Typically represents the highest block number for which the state has been initialized in the Brontes database.

**Value:** [`InitializedStateMeta`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/initialized_state.rs#L33)

- **Type:** `u8`
- **Description:** A status byte indicating the level or type of initialization that has been completed for the given block number.

### Field Details

- **State Meta**:
  - **Type:** `u8`
  - **Description:** Encodes various states of data initialization, where different values may represent different stages or types of data that have been processed and are ready for use.
