# Cex-Dex Inspector

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

# <<<<<<< HEAD

> > > > > > > refs/remotes/origin/ludwig/architecture-book

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
