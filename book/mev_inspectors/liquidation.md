# Liquidation Inspector

The Liquidation Inspector is designed to detect and analyze the profitability of liquidation events.

**What is a Liquidation?**

A liquidation occurs when a borrower's collateral is forcibly sold to repay their outstanding debt, typically when the collateral's value falls below a certain threshold.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap` or `liquidation` actions.

### Step 2: Identify Potential Liquidations

For each relevant transaction, we:

1. Split the actions into swaps and liquidations.
2. Filter out transactions with no liquidation events.

### Step 3: Analyze Liquidation Candidates

For each potential liquidation, we:

1. Collect all addresses involved in the transaction.
2. Calculate the balance changes (deltas) for all actions in the transaction.

### Step 4: Calculate Profitability

We apply specific criteria to determine the profitability of each liquidation:

1. Calculate USD value of token transfers using DEX pricing data.
2. Compute gas costs for the transaction.
3. Determine profitability by subtracting gas costs from revenue.
4. Apply a maximum profit threshold to filter out unrealistic opportunities.

### Step 5: Generate Liquidation Bundle

For confirmed liquidation opportunities:

1. Construct a `Liquidation` structure containing:

   - Liquidation transaction hash
   - Liquidation swaps
   - Liquidation events
   - Gas details

2. Create a `Bundle` with:
   - A header summarizing key information (profit, gas used, transaction hash)
   - The detailed `Liquidation` data

> **Note on Pricing:**
> The inspector uses DEX pricing data to value token transfers. If reliable pricing data is unavailable, the liquidation is flagged, and profit is set to zero to avoid false positives.
