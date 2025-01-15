# Atomic Arbitrage Inspector

The Atomic Arbitrage Inspector is designed to detect and analyze the profitability of various forms of atomic arbitrage.

**What is an atomic arbitrage?**

An atomic arbitrage is a type of arbitrage that involves multiple trades that are executed in a single transaction and result in a profit for the trader. Typically, these involve arbitraging price differences between different liquidity pools.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### Step 2: Identify and Classify Potential Atomic Arbitrages

In this step, we analyze the sequence of swaps within each transaction to identify and categorize potential arbitrages.

#### Classification Criteria

We base our classification on two main factors:

1. The number of swaps in the sequence
2. The relationships between the tokens involved in these swaps

#### Arbitrage Types

We categorize atomic arbitrages into four distinct types:

1. **Triangle**: A circular sequence of trades returning to the starting token
2. **Cross-Pair**: Trade sequences where one swap's output doesn't match the next swap's input.
3. **Stablecoin**: Arbitrages involving stablecoin pairs
4. **Long Tail**: Complex patterns not fitting the above categories

The arbitrage type will determine the filtering conditions applied subsequent steps.

> **Note:** This is by no means a comprehensive list of atomic arbitrage types. If you have discovered atomic arbitrages that do not fit these criteria, please let us know. We would love to expand our classification to include new patterns and improve our analysis.

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

### Step 4: Calculate Arbitrage PnL

We calculate the arbitrage PnL with the following steps:

1. Calculate searcher revenue: Balance deltas of searcher addresses & sibling address (e.g piggy bank address) if applicable
2. Calculate searcher cost: Gas cost & builder payment of the transaction
3. Profit = Revenue - Cost

We filter out atomic arbitrages with more than $50M in profit as this is likely a false positive caused by a bug in our DEX pricing calculation.

### Step 5: Validate Potential Arbitrages

We apply specific heuristics to filter out false positives for each identified arbitrage type. A transaction is considered a valid arbitrage if it meets any of the following conditions:

#### 1. Triangle Arbitrage

Valid if any of these conditions are met:

- Arbitrage is profitable
- Searcher has executed > 20 \* `requirement_multiplier` previous atomic arbitrages
- Searcher is manually labeled as a known atomic arbitrageur
- Transaction is private and includes a direct builder payment

#### 2. Cross-Pair Arbitrage

Valid if any of these conditions are met:

- Arbitrage is profitable
- Swaps form a stable pair at the "jump" point
- Searcher has executed > 10 \* `requirement_multiplier` previous atomic arbitrages
- Searcher is manually labeled as a known atomic arbitrageur
- Transaction is private
- Transaction includes a direct builder payment

#### 3. Stablecoin Arbitrage

Valid if any of these conditions are met:

- Arbitrage is profitable
- Any condition from Cross-Pair Arbitrage (excluding stable pair check)

#### 4. Long Tail Arbitrage

Valid if both of these conditions are met:

1. Arbitrage is profitable
2. At least one of the following is true:
   - Searcher has executed > 10 \* `requirement_multiplier` previous atomic arbitrages
   - Searcher is manually labeled as a known atomic arbitrageur
   - Transaction is private and includes a direct builder payment
   - Transaction uses a known MEV contract

> **Note on Requirement Multiplier:**
> The `requirement_multiplier` adjusts the threshold for required previous arbitrages:
>
> - 1 with reliable pricing data
> - 2 otherwise. This allows for more stringent classification when we don't have reliable pricing data.
