# Pricing

## DexPrice Table

**Table Name:** `DexPrice`

**Description:** This table stores decentralized exchange pricing data, providing transaction-level granularity for all active tokens within a block.

**Key:** `DexKey`

- **Type:** [`DexKey`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/dex.rs#L319)
- **Description:** A unique identifier combining the block number and transaction index to uniquely identify each price entry.

**Value:** `DexQuoteWithIndex`

- **Type:** [`DexQuoteWithIndex`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/dex.rs#L306)
- **Description:** Contains a vector of pairs each representing a cryptocurrency pair and its corresponding pre and post-state prices.

**Fields:**

- **tx_idx**:
  - **Type:** `u16`
  - **Description:** The index of the transaction within the block.
- **quote**:
  - **Type:** `Vec<(Pair, DexPrices)>`
  - **Description:** A list of price data for each pair, detailing the price before and after the transaction.
- **DexPrices**:
  - **Type:** [`DexPrices`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/dex.rs#L46)
  - **Description:** Detailed price information including the state before and after the transaction and the transaction method (swap or transfer).

## CexPrice Table

---

**Table Name:** `CexPrice`

**Description:** Contains price data from centralized exchanges, organized by exchange and cryptocurrency pairs.

**Key:** Block number (`u64`)

**Value:** `CexPriceMap`

- **Type:** [`CexPriceMap`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/cex/quotes/cex_quotes.rs#L73)
- **Description:** A map of exchange names to another map of currency pairs and their corresponding quotes.

**Fields:**

- **exchange**:
  - **Type:** `CexExchange`
  - **Description:** Identifies the exchange from which the price data is sourced.
- **Pair**:
  - **Type:** `Pair`
  - **Description:** The cryptocurrency pair (e.g., BTC/USD) for which the price is provided.
- **CexQuote**:
  - **Type:** [`Vec<CexQuote>`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/cex/quotes/cex_quotes.rs#L539)
  - **Description:** A list of bid and ask prices along with the amounts and timestamp of the quote.

## CexTrades Table

---

**Table Name:** `CexTrades`

**Description:** Holds trade data from centralized exchanges, providing insights into market transactions.

**Key:** Block number (`u64`)

**Value:** `CexTradeMap`

- **Type:** [`CexTradeMap`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/cex/trades/cex_trades.rs#L19)
- **Description:** A map organizing trade data by exchange and currency pairs, detailing each trade's price and amount.

**Fields:**

- **exchange**:
  - **Type:** `CexExchange`
  - **Description:** Identifies the exchange where the trade occurred.
- **Pair**:
  - **Type:** `Pair`
  - **Description:** The cryptocurrency pair involved in the trade.
- **CexTrades**:

  - **Type:** [`Vec<CexTrades>`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/cex/trades/cex_trades.rs#L19)
  - **Description:** Detailed records of each trade, including the timestamp, price, and amount.
