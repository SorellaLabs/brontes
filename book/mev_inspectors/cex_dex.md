# Cex-Dex Inspector

The Cex-Dex inspector identifies arbitrage between centralized and decentralized exchanges. While on-chain DEX trades are visible, CEX trades must be inferred. Using available CEX trade data the inspector estimates likely CEX trade prices to approximate the full arbitrage strategy and its profitability.

**What is Cex-Dex Arbitrage?**

Centralized exchanges (CEX) and decentralized exchanges (DEX) operate on fundamentally different time scales. CEX function in continuous time, allowing trades to be executed at any moment without interruption. In contrast, DEX operate in discrete time intervals, as trades are only executed upon inclusion in a new block - leaving prices stale in between blocks. Consequently, DEX prices consistently lag behind the more frequently updated CEX prices, creating arbitrage opportunities between the two exchange types.

## Methodology

### Step 1: Identify Potential Arbitrage Transactions

First, the inspector collects all block transactions involving `swap`, `transfer`, `eth_transfer`, `aggregator_swap` actions.

Then, for each transaction it:

1. Discards transactions if it's a solver settlements or from a known DeFi automation bot.
2. Extract DEX swaps and transfers.
3. If no swaps are found, attempt to reconstruct swaps from transfers.
4. Discard transactions that represent atomic arbitrage (where trades form a closed loop).

### Step 2: Merge Sequential Swaps

<div style="text-align: center;">
 <img src="cex-dex/swap-merging.png" alt="Swap Merging" style="border-radius: 20px; width: 400px; height: auto;">
</div>

We merge sequential swaps to match on-chain routes with off-chain markets. Here's why:

- On-chain and off-chain liquidity often differ. For example, PEPE-WETH might be the most liquid pair on-chain, while PEPE-USDT dominates off-chain.
- Arbitrageurs might swap PEPE-WETH then WETH-USDT on-chain to arbitrage against the PEPE-USDT off-chain market.
- By merging these on-chain swaps (PEPE-WETH-USDT into PEPE-USDT), we align our analysis with the actual off-chain trade.

Our `merge_possible_swaps` function combines these sequential swaps, allowing us to evaluate CEX prices more precisely.

### Step 3: CEX Price Estimation

To estimate the CEX price the arbitrageur traded at, we use two distinct methods:

#### Dynamic Time Window Volume Weighted Markouts

This method involves calculating a Volume Weighted Average Price (VWAP) for trades that occur within a dynamic time window centered about the block time. This time window is extended until their is sufficient trading volume to clear the arbitrage opportunity. The time window is dynamic because depending on the competitivness of the pair, the arbitrageur will trade at different times. For a very competitive arbitrage, the arbitrageur is incentivized to trade very close to the block time because their is high uncertainty on if their arbitrage will be included. On the other hand for a less competitive arbitrage, the arbitrageur can trade further away from the block time because their is less uncertainty on if their arbitrage will be included and can therefore capture more of the price discrepancy. Furthermore, we realized that very tight time windows don't work for less competitive arbitrages because their simply isn't enough volume off chain to clear the arbitrage, however it is very clear that the arbitrage is happening. Therefore, we need to extend the time window to capture the arbitrage.

##### Determining the Time Window

<div style="text-align: center;">
 <img src="cex-dex/default-time-window.png" alt="Dynamic Time Window" style="border-radius: 20px; width: 550px; height: auto;">
</div>

1. Set a default time window of 50 milliseconds before & after the block time.
2. Collect all trades within this window and calculate the total volume.
3. If volume is insufficient, dynamically extend the time window. First extending the time window post block time up to 300 milliseconds, in increments of 10 milliseconds. Then extending both the pre & post block time windows in increments of 10 milliseconds up to the maximum time window of -5 +8 seconds. For reference these time windows are fully configurable.

<div style="text-align: center;">
 <img src="cex-dex/first-extension-time-window.png" alt="Dynamic Time Window Initial Extension" style="border-radius: 20px; width: 550px; height: auto;">
</div>

<div style="text-align: center;">
 <img src="cex-dex/final-time-window.png" alt="Dynamic Time Window Initial Extension" style="border-radius: 20px; width: 550px; height: auto;">
</div>

##### Accounting for execution risk

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
