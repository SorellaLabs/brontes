# JIT Liquidity and JIT CexDex Inspectors

These inspectors are designed to detect and analyze the profitability of Just-In-Time (JIT) Liquidity and JIT CexDex opportunities.

## What is JIT Liquidity?

JIT Liquidity is a type of MEV where a trader sandwiches a large swap on a concentrated liquidity pool by providing & subsequently removing liquidity. It unfolds in three steps:

1. **Front-run:** The attacker provides extremely concentrated liquidity at the ticks that will be active during the large swap.
2. **Victim Transaction:** The victim executes their swap.
3. **Back-run:** The attacker removes the liquidity immediately after the victim's tx, collecting the fees from the swap.

## What is JIT CexDex?

JIT CexDex, a variant of JIT Liquidity attacks, exploits the price discrepancies between centralized exchanges (CEX) and decentralized exchanges (DEX). Nearly all JITs observed in practice are JIT CexDex. It occurs when:

1. There's is a price discrepancy between a CEX & a DEX that is within the fee bound so executing an arbitrage on the DEX is not profitable after accounting for the swap fee.
2. There is a CEX DEX opportunity, but the volume required to execute the arbitrage & rebalance the pool back to the true price is less than the volume of an incoming user swap on the DEX, so the attacker can extract more value by being a maker for the swap as opposed to executing the arbitrage directly against the pool.

In this scenario, market makers provide liquidity for the user swap, effectively arbitraging the price discrepancy between the CEX & DEX while receiving, instead of incurring, the DEX swap fee.

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

   - Map addresses (contract & EOA) to their most recent transaction hash

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

### Step 2: Partitioning & Filter Possible JITs

Here's how partitioning works:

- We iterate through victim sets in each JIT.
- Empty victim sets signal a break in the JIT.
- We create new `PossibleJit` structs at these breaks.

<div style="text-align: center;">
 <img src="sandwich/partition-sandwich.png" alt="Possible Sandwich Partitioning" style="border-radius: 20px; width: 600px; height: auto;">
</div>

> **Note:** Our partitioning assumes attackers maximize efficiency. Multiple attacker transactions without intervening victims may lead to unexpected results. If you find examples breaking this assumption, please report them for a bounty.

**Filter and Refine**:

- We filter out `PossibleJit` with more than 10 victim sets or 20 victims
- We ensure that the frontrun in the set includes a `mint` action and the backrun includes a `burn` action.

### Step 3: Analyze JIT Candidates

For each `PossibleJit`, we:

1. Check for recursive JIT patterns, verifying:

   - Mint and Burn Sequence: Logical order of liquidity additions and removals
   - Account Consistency: Same account for all transactions
   - Token Alignment: Matching tokens in mints and burns

2. If a recursive pattern is detected, initiate recursive analysis. Otherwise, proceed with:
   - Splitting actions into mints, burns, and other transfers
   - Verifying presence of both mints and burns
   - Ensuring mints and burns are for the same pools

#### Recursive JIT Verification

For non standard JIT patterns, we employ a recursive strategy:

<div style="text-align: center;">
 <img src="jit/recursive-check.png" alt="Recursive JIT Split" style="border-radius: 20px; width: 600px; height: auto;">
</div>

1. The process stops after 10 recursive iterations.

2. We apply two types of "shrinking":

   **Back Shrink**:

   - Remove the last victim set
   - Use the last front-run as the new back-run
   - Recalculate the JIT opportunity

   **Front Shrink**:

   - Remove the first victim set
   - Remove the first front-run transaction
   - Retain the original back-run
   - Recalculate the JIT opportunity

3. We continue this process as long as:
   - There's more than one front-run transaction
   - Victim sets aren't empty
   - At least one victim has non-empty actions

### Step 4: Validate JIT Opportunities

For confirmed JIT bundles:

1. Calculate searcher revenue: Balance deltas of searcher addresses & sibling address (e.g piggy bank address) if applicable
2. Calculate searcher cost: Sum of gas costs for all attacker transactions
3. Profit = Revenue - Cost
4. Applying a maximum profit threshold

### Step 5: Generate JIT Bundle

For confirmed opportunities:

1. Construct a `JitLiquidity` structure with detailed transaction information
2. Create a `Bundle` with a summary header and `JitLiquidity` data
3. For recursive analyses, deduplicate results and keep the largest JIT bundle

### Step 6: Identify JIT CexDex

For validated JIT opportunities, we perform additional checks:

1. Verify if the searcher is labeled as a known CexDex arbitrageur.
2. Analyze the swaps to detect CEX-DEX arbitrage patterns.
3. Compare DEX swaps with CEX trade data to confirm price discrepancies.
