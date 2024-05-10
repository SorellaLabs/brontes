# Classification Tables

## AddressToProtocolInfo Table

---

**Table Name:** `AddressToProtocolInfo`

**Description:** Stores mappings of blockchain addresses to specific protocol info, used by the classifier dispatch to decode and normalize traces.

**Key:** Address

- **Type:** `Address`
- **Description:** Contract Address.

**Value:** [`ProtocolInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_to_protocol_info.rs#L27)

- **Description:** Contains information linking an address to a protocol and its associated tokens.

**Fields:**

- **protocol**:
  - **Type:** `Protocol`
  - **Description:** The protocol associated with the address.
- **token0**, **token1**, **token2**, **token3**, **token4**:
  - **Type:** `Address`
  - **Description:** Addresses of tokens associated with the contract, where `token0` and `token1` are mandatory and others are optional. If the contract doesn't contain a token the addresses are set to the zero address.
- **curve_lp_token**:
  - **Type:** `Option<Address>`
  - **Description:** Address of the Curve liquidity pool token, if applicable.
- **init_block**:
  - **Type:** `u64`
  - **Description:** The block at which the contract was created.

## TokenDecimals Table

---

**Table Name:** `TokenDecimals`

**Description:** Provides token decimals and symbols.

**Key:** Address

- **Type:** `Address`
- **Description:** Token Address.

**Value:** [`TokenInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/token_info.rs#L113)

- **Description:** Contains token decimals and symbols.

**Fields:**

- **decimals**:
  - **Type:** `u8`
  - **Description:** Token decimals.
- **symbol**:
  - **Type:** `String`
  - **Description:** Token symbol.
