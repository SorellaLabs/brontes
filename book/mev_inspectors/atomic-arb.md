# Atomic Arbitrage

The Atomic Arbitrage Inspector is designed to detect and analyze the profitability of various forms of atomic arbitrage.

**What is an atomic arbitrage?**

An atomic arbitrage is a type of arbitrage that involves multiple trades that are executed atomically and result in a profit for the trader. Typically, atomic arbitrages involve arbitraging price differences between different liquidity pools.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### Step 2: Identify Potential Atomic Arbitrage and Classify it's Type

We analyze the sequence of swaps in each transaction to identify and categorize potential atomic arbitrages. Our classification is based on the number of swaps and the relationships between the tokens involved.

We classify atomic arbitrages in these distinct types:

- Triangle,
- CrossPair
- StablecoinArb,
- LongTail

#### For Zero or One Swap

- Not considered an arbitrage opportunity. We move to the next transaction.

#### For Two Swaps

1. **Triangle Arbitrage**

   - Condition: Input token of Swap 1 matches output token of Swap 2, and swaps are continuous.

   ```ignore
   Swap 1: WETH → USDC
   Swap 2: USDC → WETH
   ```

2. **Stablecoin Arbitrage**

   - Triangle (stablecoins):

   ```ignore
   Swap 1: USDC → USDT
   Swap 2: USDT → USDC
   ```

   - Non-Triangle (input of Swap 1 and output of Swap 2 form a stable pair):

   ```ignore
   Swap 1: USDC → WETH
   Swap 2: WETH → USDT
   ```

3. **Cross-Pair Arbitrage**

   - Condition: The sequence starts and ends with the same token, but there's a break in continuity where the second swap's input token doesn't match first swap's output token.

   ```ignore
   Swap 1: WETH → USDC
   Swap 2: WBTC → WETH
   ```

4. **Long Tail**
   - Any swap pattern not fitting the above categories.

#### For Three or More Swaps

1. **Stablecoin Arbitrage**

   - Condition: First and last tokens form a stable pair.

   ```ignore
   Swap 1: USDC → WETH
   Swap 2: WETH → WBTC
   Swap 3: WBTC → DAI
   ```

2. **Cross-Pair Arbitrage**

   - Condition: The sequence starts and ends with the same token, but there's a break in continuity where one swap's output doesn't match the next swap's input.

   ```ignore
   Example:
   Swap 1: WETH → USDC
   Swap 2: WBTC → DAI
   Swap 3: DAI  → WETH
   ```

3. **Triangle Arbitrage**

   - Condition: All swaps are continuous and the swap sequence ends with the starting token.

   ```ignore
   Swap 1: WETH → USDC
   Swap 2: USDC → WBTC
   Swap 3: WBTC → WETH
   ```

4. **Long Tail**
   - Any swap pattern not fitting the above categories.

> **Note on Stable Pair Identification:**
> We consider two tokens a stable pair if both are stablecoins of the same type. Our definition of stablecoins extends beyond just USD-pegged tokens:
>
> - USD stablecoins (e.g., USDC, USDT, DAI)
> - EURO stablecoins (e.g., EURS, EURT)
> - GOLD stablecoins (e.g., PAXG, XAUT)

### Step 4: Validate Profitability

We consider an arbitrage valid if:

1. It's profitable after gas costs, or
2. It meets specific criteria based on the arbitrage type and transaction characteristics

### Step 5: Price Verification

We compare the effective swap rates with external price data to ensure the arbitrage is genuine and not a result of manipulated on-chain prices.

## Key Components

- **PossibleSandwich**: Stores details of potential arbitrages, including involved addresses and transaction hashes.
- **AtomicArbType**: Enum representing different types of atomic arbitrages.
- **Profit Calculation**: We calculate profit by subtracting gas costs from the revenue in USD.

## Note on Filtering

We apply different filtering criteria based on the arbitrage type and transaction characteristics. This helps us focus on genuine arbitrage opportunities and reduce false positives.

For detailed implementation, refer to the [source code](https://github.com/YourRepo/AtomicArbInspector).
