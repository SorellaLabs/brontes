# Cex-Dex Inspector

The Cex-Dex inspector identifies arbitrage between centralized and decentralized exchanges. While on-chain DEX trades are visible, CEX trades must be inferred. Using available CEX trade data, the inspector estimates likely CEX trade prices. This allows for approximating the full arbitrage strategy and its potential profitability.

**What is Cex-Dex Arbitrage?**

Cex-Dex arbitrage involves exploiting price differences between centralized and decentralized exchanges. This arbitrage opportunity arises because centralized exchange operate in near continuous time while decentralized exchange operate in discrete time, updating only when a new block is produced. This causes the DEX price to lag behind the CEX price, creating opportunities for arbitrage.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions on decentralized exchanges.

### Step 2: Identify Potential Cex-Dex Arbitrage

For each transaction:

1. Filter out solver settlement and DeFi automation transactions.
2. Extract DEX swaps and transfers from the transaction.
3. If no DEX swaps are found, attempt to convert transfers to swaps for labeled CEX-DEX bots.
4. Filter out triangular arbitrage (where the first and last token in the swap sequence are the same).

### Step 3: Estimate Centralized Exchange Prices

For each DEX swap, the inspector estimates corresponding CEX prices using two methods:

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

## Markout with trades

We currently operate an extremely optimistic model to compute the centralized exchange price.

Given a set of trades over a time window relative to the block time. We sort all trades by price and then we jump to the index that represent our quality percentage. We iterate over these trades, to form the volume weighted average price for the trades that we take (up to the trade size necessary to hedge)

This is overly optimistic and has extreme lookahead bias because it assumes they have strong signal on the trade price over that time window and are able to select only the trades that maximizes their arbitrage PNL.

## Dynamic Time Window Volume Weighted Markout

1. We establish a default time window. Say: -0.5 before the block | +2 after the block

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
