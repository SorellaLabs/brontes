# JIT Liquidity and JIT CexDex Inspectors

These inspectors are designed to detect and analyze the profitability of Just-In-Time (JIT) Liquidity and JIT CexDex opportunities.

## What is JIT Liquidity?

JIT Liquidity is a type of MEV where a trader identifies a large swap on a concentrated liquidity pool and sandwiches it to provide liquidity, then removes it immediately after the swap. It unfolds in three steps:

1. **Front-run:** The attacker provides extremely concentrated liquidity at the ticks that will be active during the large swap.
2. **Victim Transaction:** The victim executes their swap.
3. **Back-run:** The attacker removes the liquidity immediately after the victim's transaction, collecting the fees from the swap.

## What is JIT CexDex?

TODO:

- there is a third type where the pool can be arbed against the CEX, but their is a swap that goes in the opposite direction so they will fill by JITing to arb the user & then backrun the swap arbitraging the pool back to true price.

JIT CexDex is a specialized form of JIT Liquidity that exploits price discrepancies between centralized exchanges (CEX) and decentralized exchanges (DEX). It occurs when:

1. There's is a price discrepancy between a centralize exchange (CEX) and a decentralized exchange (DEX) but the price is within the fee bound so executing an arbitrage on the DEX is not profitable after the swap fee.
2. There is a CEX DEX opportunity, but the volume required to execute the arbitrage is less than an incoming swap on the DEX, so the attacker can extract more value by being a maker for the swap that executing the arbitrage directly against the pool.

In this scenario, market makers provide liquidity for the user swap, effectively arbitraging the price discrepancy between the CEX & DEX while receiving instead of paying the DEX swap fee.

## Methodology

### Step 1: Identify Potential JIT Opportunities

We analyze the transaction tree to identify potential JIT Liquidity scenarios checking for:

- Repeated transactions from the same account
- Repeated calls to the same contract

The `PossibleJit` type represents each potential opportunity:

```rust,ignore
pub struct PossibleJit {
    pub eoa: Address,
    pub frontrun_txes: Vec<B256>,
    pub backrun_tx: B256,
    pub executor_contract: Address,
    pub victims: Vec<Vec<B256>>,
}
```

This struct holds the attacker's address, frontrun and backrun transactions, the contract used, and sets of victim transactions.

#### How It Works

Our algorithm constructs the largest possible JIT scenarios by identifying duplicate addresses. Here's the process:

1. **Track Duplicates**:

   - Map addresses (contract or EOA) to their most recent transaction hash

2. **Build Victim Sets**:

   - For each transaction, track potential victims (transactions that occur after it)

3. **Construct PossibleJit**:

   - When we encounter a duplicate address, we create or update a `PossibleJit`:
     a) For the first duplicate:
     - Create a new PossibleJit
     - Set the previous transaction as the frontrun
     - Set the current transaction as the backrun
     - Add intervening transactions as victims
       b) For subsequent duplicates:
     - Add the previous transaction to possible frontruns
     - Update the backrun to the current transaction
     - Add the new set of victims

4. **Filter and Refine**:
   - We filter out scenarios with more than 10 victim sets or 20 victims
   - We ensure that the set includes both mint and burn operations

This approach allows us to capture a wide range of JIT Liquidity patterns, from simple to complex multi-step operations. The resulting list of `PossibleJit` structures represents the most comprehensive JIT scenarios in the block, ready for further analysis in subsequent steps.

### Step 2: Analyze JIT Candidates

For each `PossibleJit`, we:

1. Retrieve detailed transaction information for front-runs, back-runs, and victims.
2. Analyze the actions within these transactions (mints, burns, swaps).
3. Calculate potential profit, considering gas costs and other factors.

### Step 3: Validate JIT Opportunities

We apply specific criteria to filter out false positives:

1. Ensure the presence of both mints (in front-runs) and burns (in back-runs).
2. Verify that mints and burns are for the same tokens and pools.
3. Check for profitability after accounting for gas costs.

### Step 4: Identify JIT CexDex (for applicable cases)

For validated JIT opportunities, we perform additional checks:

1. Verify if the searcher is labeled as a known CexDex arbitrageur.
2. Analyze the swaps to detect CEX-DEX arbitrage patterns.
3. Compare DEX swaps with CEX trade data to confirm price discrepancies.

### Step 5: Calculate Profit and Generate Bundle

For confirmed JIT and JIT CexDex opportunities:

1. Calculate the final profit, considering all relevant factors.
2. Generate a `Bundle` structure containing detailed information about the MEV opportunity.
