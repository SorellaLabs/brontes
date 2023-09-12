pub mod atomic_backrun;
pub mod sandwich;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc
};

use clickhouse::Row;
use malachite::Rational;
use poirot_labeller::Metadata;
use poirot_types::{
    normalized_actions::Actions,
    tree::{GasDetails, TimeTree},
    ToScaledRational, TOKEN_TO_DECIMALS
};
use reth_primitives::{Address, H256, U256};
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    pub tx_hash:      H256,
    pub contract:     Address,
    // gas related
    pub gas_details:  GasDetails,
    pub priority_fee: u64,

    // results
    pub block_appearance_revenue_usd: f64,
    pub block_finalized_revenue_usd:  f64,

    pub block_appearance_profit_usd: f64,
    pub block_finalized_profit_usd:  f64
}

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>
    ) -> Vec<ClassifiedMev>;

    /// Calculates the swap deltas. if transfers are also passed in. we also
    /// move those around accordingly.
    fn calculate_swap_deltas(
        &self,
        actions: &Vec<Vec<Actions>>
    ) -> HashMap<Address, HashMap<Address, Rational>> {
        // address and there token delta's
        let mut deltas = HashMap::new();
        for action in actions.into_iter().flatten() {
            if let Actions::Swap(swap) = action {
                let Some(decimals_in) = TOKEN_TO_DECIMALS.get(&swap.token_in.0) else {
                    error!(missing_token=?swap.token_in, "missing token in token to decimal map");
                    continue
                };

                let Some(decimals_out) = TOKEN_TO_DECIMALS.get(&swap.token_out.0) else {
                    error!(missing_token=?swap.token_in, "missing token in token to decimal map");
                    continue
                };

                let adjusted_in = -swap.amount_in.to_scaled_rational(*decimals_in);
                let adjusted_out = swap.amount_out.to_scaled_rational(*decimals_out);

                match deltas.entry(swap.call_address) {
                    Entry::Occupied(mut o) => {
                        let inner: &mut HashMap<Address, Rational> = o.get_mut();

                        apply_entry(swap.token_in, adjusted_in, inner);
                        apply_entry(swap.token_out, adjusted_out, inner);
                    }
                    Entry::Vacant(v) => {
                        let mut default = HashMap::default();
                        default.insert(swap.token_in, adjusted_in);
                        default.insert(swap.token_out, adjusted_out);

                        v.insert(default);
                    }
                }
            } else if let Actions::Transfer(transfer) = action {
                let Some(decimals) = TOKEN_TO_DECIMALS.get(&transfer.token.0) else {
                    error!(missing_token=?transfer.token, "missing token in token to decimal map");
                    continue
                };

                let adjusted_amount = transfer.amount.to_scaled_rational(*decimals);

                let from_token_map = deltas.entry(transfer.from).or_default();
                apply_entry(transfer.token, -adjusted_amount.clone(), from_token_map);

                let to_token_map = deltas.entry(transfer.to).or_default();
                apply_entry(transfer.token, adjusted_amount, to_token_map);
            }
        }

        deltas
    }
}

fn apply_entry(token: Address, amount: Rational, token_map: &mut HashMap<Address, Rational>) {
    match token_map.entry(token) {
        Entry::Occupied(mut o) => {
            *o.get_mut() += amount;
        }
        Entry::Vacant(v) => {
            v.insert(amount);
        }
    }
}
