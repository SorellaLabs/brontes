# Classification Tables

## AddressToProtocolInfo Table

**Table Name:** `AddressToProtocolInfo`

**Description:** Stores mappings of blockchain addresses to specific protocol information, crucial for decoding and normalization processes related to different blockchain protocols.

**Key:** Address

- **Type:** `Address`
- **Description:** Blockchain address that may be associated with a protocol.

**Value:** [`ProtocolInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/address_to_protocol_info.rs#L27)

- **Description:** Contains information linking an address to a protocol and its associated tokens.

**Fields:**

- **protocol**:
  - **Type:** `Protocol`
  - **Description:** The protocol associated with the address.
- **token0**, **token1**, **token2**, **token3**, **token4**:
  - **Type:** `Address`
  - **Description:** Addresses of tokens associated with the protocol, where `token0` and `token1` are mandatory and others are optional.
- **curve_lp_token**:
  - **Type:** `Option<Address>`
  - **Description:** Address of the Curve liquidity pool token, if applicable.
- **init_block**:
  - **Type:** `u64`
  - **Description:** The blockchain block number at which the protocol was initialized or first interacted with.

## TokenDecimals Table

---

**Table Name:** `TokenDecimals`

**Description:** Provides decimal precision for various tokens, crucial for financial calculations and accurate representation of token amounts.

**Key:** Address

- **Type:** `Address`
- **Description:** Blockchain address of the token.

**Value:** [`TokenInfo`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/token_info.rs#L113)

- **Description:** Contains decimal information and the symbol for the token.

**Fields:**

- **decimals**:
  - **Type:** `u8`
  - **Description:** Number of decimal places used to specify the token's smallest unit.
- **symbol**:
  - **Type:** `String`
  - **Description:** The symbol or short representation of the token, commonly used in exchanges and wallets.
