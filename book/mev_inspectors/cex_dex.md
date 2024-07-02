# Cex-Dex Inspector

The Cex-Dex inspector identifies arbitrage between centralized and decentralized exchanges. While on-chain DEX trades are visible, CEX trades must be inferred. Using available CEX trade data the inspector estimates likely CEX trade prices to approximate the full arbitrage strategy and its profitability.

**What is Cex-Dex Arbitrage?**

Centralized exchanges (CEX) and decentralized exchanges (DEX) operate on fundamentally different time scales. CEX function in continuous time, allowing trades to be executed at any moment without interruption. In contrast, DEX operate in discrete time intervals, as trades are only executed upon inclusion in a new block - leaving prices stale in between blocks. Consequently, DEX prices consistently lag behind the more frequently updated CEX prices, creating arbitrage opportunities between the two exchange types.

## Methodology

### Step 1: Identify Potential Arbitrage Transactions

The inspector works in two phases:

First, it collects all block transactions involving `swap`, `transfer`, `eth_transfer`, `aggregator_swap` actions.

Then, for each transaction:

1. Discard transactions if it's a solver settlements or from a known DeFi automation bot.
2. Extract DEX swaps and transfers.
3. If no swaps are found, attempt to reconstruct swaps from transfers.
4. Discard transactions that represent atomic arbitrage (where trades form a closed loop).

TODO: We are currently only attempting to create swaps from transfers if there are no identified swaps. We should instead do this for all transfers but apply a stricter methodology to identify them.

FOR API: Intermediary skip only happens when we have A->B->C which will change what is stored in the bundle data vs the tree, so you know that you should show the A->B->C on a single page for the details when these two are different.

### Step 2: CEX Price Estimation

1. Merge Sequential Swaps

- We first use the `merge_possible_swaps` function to combine sequential swaps via intermediaries into direct swaps where possible.
- This process identifies cases where two swaps (A->B and B->C) can be represented as a single swap (A->C).
- Merging swaps allows us to evaluate CEX prices more accurately, especially when there's a direct trading pair on centralized exchanges.

For each DEX swap, the inspector estimates corresponding CEX prices using two methods:

## Dynamic Time Window Volume Weighted Markout

We establish a default time window. Say: -0.5 before the block | +2 after the block

For that given default interval, we collect all trades & evaluate total volume. If volume is insufficient we will extend the post-block time interval by increments of 0.1.

Once our time window has been extended to -0.5 | +3 post, if we still don't have sufficient volume we start extending our pre block by increments of 0.1 until we reach - 2. We do so while incrementing post block up to +4.

Once our time window has been extended to -2 | +4 if we still don't have we will increment our post window up to +5. If there is still insufficient volume we will increment our pre window to -3.

We will repeat this extension if volume is insufficient until we reach our max interval. E.g: -3 | + 5

## Accounting for execution risk

- **Risk of Price Movements**:

### Bi-Exponential Decay Function

The bi-exponential decay function is used to assign different weights to trades occurring before and after the block time. This approach allows us to skew the weighting to favour the post block time trades in consideration of the fact that arbitrageurs gain certainty in their DEX execution.

$$
Weight(t) =
\begin{cases}
e^{-\lambda_{pre} \cdot (BlockTime - t)} & \text{if } t < BlockTime \\\\
e^{-\lambda_{post} \cdot (t - BlockTime)} & \text{if } t \geq BlockTime
\end{cases}
$$

Where:

- \\( \text{t} \\) is the timestamp of each trade.
- \\( \text{BlockTime} \\) is the first time the block has been seen on the p2p network.
- \\( \lambda\_{pre} \\) is the decay rate before the block time.
- \\( \lambda\_{post} \\) is the decay rate after the block time.

Proposed values are [here](https://www.desmos.com/calculator/7ktqmde9ab)

### Adjusted Volume Weighted Average Price (VWAP)

To integrate both volume information and the bi-exponential time decay into the VWAP, we adjust the calculation as follows:

$$
AdjustedVWAP = \frac{\sum (Price_i \times Volume_i \times TimingWeight_i)}{\sum (Volume_i \times TimingWeight_i)}
$$

#### A. Time Window Volume Weighted Average Markout (VWAM)

1. Set a default time window around the block timestamp.
2. Collect all trades within this window and calculate the total volume.
3. If volume is insufficient, dynamically extend the time window.
4. Apply a bi-exponential decay function to weight trades based on their temporal proximity to the block timestamp.
5. Calculate an Adjusted Volume Weighted Average Price (VWAP) using these weights.

#### B. Optimistic VWAP

1. Collect all trades within a set time window.
2. Sort trades by price and select the most favorable trades up to the required volume.
3. Calculate VWAP based on these selected trades.

### Step 4: Calculate Potential Arbitrage Profits

For each swap and CEX price estimate:

1. Calculate the price difference between DEX and CEX.
2. Estimate potential profit for both buying on DEX and selling on CEX, and vice versa.
3. Calculate profits using both mid-price and ask-price scenarios.

### Step 5: Aggregate and Analyze Results

1. Calculate profits for each CEX individually and for a global VWAM across all exchanges.
2. Determine the most profitable route across all exchanges.
3. Calculate optimistic profits based on the Optimistic VWAP.

### Step 6: Account for Gas Costs

Subtract the transaction's gas cost from the calculated profits for each scenario.

### Step 7: Validate and Filter Potential Arbitrages

A transaction is considered a valid Cex-Dex arbitrage if it meets any of the following conditions:

1. Profitable based on global VWAM or optimistic estimates.
2. Profitable on multiple exchanges.
3. Executed by an address with significant history of Cex-Dex arbitrage (>40 previous trades).
4. Labeled as a known Cex-Dex arbitrageur.
5. Is a private transaction with direct builder payment.
6. Uses a known MEV contract.
7. Shows significant profit on a single exchange (excluding stable coin pairs).

### Step 8: Handle Edge Cases and Outliers

1. Filter out high-profit outliers (>$10,000 profit) on specific exchanges (Kucoin, Okex) to avoid false positives.
2. Apply stricter validation for stable coin pair arbitrages.

### Step 9: Prepare Final Output

For validated Cex-Dex arbitrages, compile detailed information including:

1. Transaction details (hash, gas costs, etc.)
2. DEX swap details
3. Estimated CEX prices and trade details
4. Calculated profits for various scenarios (global VWAM, per-exchange, optimistic)
5. Time windows used for price estimations

The inspector outputs this information as a `Bundle` containing `BundleData::CexDex` for further analysis and reporting.
