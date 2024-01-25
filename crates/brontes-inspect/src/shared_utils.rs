use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::U256;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    extra_processing::Pair,
    normalized_actions::{Actions, NormalizedTransfer},
    ToScaledRational,
};
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::error;

use crate::MetadataCombined;

#[derive(Debug)]
pub struct SharedInspectorUtils<'db, DB: LibmdbxReader> {
    pub(crate) quote: Address,
    pub(crate) db:    &'db DB,
}

impl<'db, DB: LibmdbxReader> SharedInspectorUtils<'db, DB> {
    pub fn new(quote_address: Address, db: &'db DB) -> Self {
        SharedInspectorUtils { quote: quote_address, db }
    }
}

type SwapTokenDeltas = HashMap<Address, HashMap<Address, Rational>>;

impl<DB: LibmdbxReader> SharedInspectorUtils<'_, DB> {
    /// Calculates the swap deltas.
    pub(crate) fn calculate_token_deltas(&self, actions: &Vec<Vec<Actions>>) -> SwapTokenDeltas {
        let mut transfers = Vec::new();
        // Address and there token delta's
        let mut deltas = HashMap::new();

        for action in actions.into_iter().flatten() {
            // If the action is a swap, get the decimals to scale the amount in and out
            // properly.
            if let Actions::Swap(swap) = action {
                let Ok(Some(decimals_in)) = self.db.try_get_token_decimals(swap.token_in) else {
                    error!(?swap.token_in, "token decimals not found");
                    continue;
                };
                let Ok(Some(decimals_out)) = self.db.try_get_token_decimals(swap.token_out) else {
                    error!(?swap.token_out, "token decimals not found");
                    continue;
                };

                let adjusted_in = -swap.amount_in.to_scaled_rational(decimals_in);
                let adjusted_out = swap.amount_out.to_scaled_rational(decimals_out);

                // we track the address deltas so we can apply transfers later on the profit
                // collector
                if swap.from == swap.recipient {
                    let entry = deltas.entry(swap.from).or_insert_with(HashMap::default);
                    apply_entry(swap.token_out, adjusted_out, entry);
                    apply_entry(swap.token_in, adjusted_in, entry);
                } else {
                    let entry_recipient = deltas.entry(swap.from).or_insert_with(HashMap::default);
                    apply_entry(swap.token_in, adjusted_in, entry_recipient);

                    let entry_from = deltas
                        .entry(swap.recipient)
                        .or_insert_with(HashMap::default);
                    apply_entry(swap.token_out, adjusted_out, entry_from);
                }

            // If there is a transfer, push to the given transfer addresses.
            } else if let Actions::Transfer(transfer) = action {
                transfers.push(transfer);
            }
        }

        self.transfer_deltas(transfers, &mut deltas);

        // Prunes proxy contracts that receive and immediately send, like router
        // contracts
        deltas.iter_mut().for_each(|(_, v)| {
            v.retain(|_, rational| (*rational).ne(&Rational::ZERO));
        });

        deltas
    }

    /// Calculates the usd delta by address
    pub fn usd_delta_by_address(
        &self,
        tx_position: usize,
        post_state: bool,
        deltas: &SwapTokenDeltas,
        metadata: Arc<MetadataCombined>,
        cex: bool,
    ) -> Option<HashMap<Address, Rational>> {
        let mut usd_deltas = HashMap::new();

        for (address, inner_map) in deltas {
            for (token_addr, amount) in inner_map {
                let pair = Pair(*token_addr, self.quote);
                let price = if cex {
                    // Fetch CEX price
                    metadata.cex_quotes.get_binance_quote(&pair)?.best_ask()
                } else {
                    metadata
                        .dex_quotes
                        .price_at_or_before(pair, tx_position)
                        .map(|price| if post_state { price.post_state } else { price.pre_state })?
                };

                let usd_amount = amount.clone() * price.clone();

                *usd_deltas.entry(*address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        Some(usd_deltas)
    }

    pub fn calculate_dex_usd_amount(
        &self,
        block_position: usize,
        post_state: bool,
        token_address: Address,
        amount: U256,
        metadata: &Arc<MetadataCombined>,
    ) -> Option<Rational> {
        let Ok(Some(decimals)) = self.db.try_get_token_decimals(token_address) else {
            error!("token decimals not found for calcuate dex usd amount");
            return None
        };
        if token_address == self.quote {
            return Some(amount.to_scaled_rational(decimals))
        }

        let pair = Pair(token_address, self.quote);
        Some(
            metadata
                .dex_quotes
                .price_at_or_before(pair, block_position)
                .map(|price| if post_state { price.post_state } else { price.pre_state })?
                * amount.to_scaled_rational(decimals),
        )
    }

    pub fn get_dex_usd_price(
        &self,
        block_position: usize,
        post_state: bool,
        token_address: Address,
        metadata: Arc<MetadataCombined>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);
        metadata
            .dex_quotes
            .price_at_or_before(pair, block_position)
            .map(|price| if post_state { price.post_state } else { price.pre_state })
    }

    pub fn profit_collectors(&self, addr_usd_deltas: &HashMap<Address, Rational>) -> Vec<Address> {
        addr_usd_deltas
            .iter()
            .filter_map(|(addr, value)| (*value > Rational::ZERO).then(|| *addr))
            .collect()
    }

    /// Account for all transfers that are in relation with the addresses that
    /// swap, so we can track the end address that collects the funds if it is
    /// different to the execution address
    fn transfer_deltas(&self, _transfers: Vec<&NormalizedTransfer>, _deltas: &mut SwapTokenDeltas) {
        // currently messing with price
        // for transfer in transfers.into_iter() {
        //     // normalize token decimals
        //     let Ok(Some(decimals)) =
        // self.db.try_get_token_decimals(transfer.token) else {
        //         error!("token decimals not found");
        //         continue;
        //     };
        //     let adjusted_amount =
        // transfer.amount.to_scaled_rational(decimals);
        //
        //     // fill forward
        //     if deltas.contains_key(&transfer.from) {
        //         // subtract balance from sender
        //         let mut inner = deltas.entry(transfer.from).or_default();
        //
        //         match inner.entry(transfer.token) {
        //             Entry::Occupied(mut o) => {
        //                 if *o.get_mut() == adjusted_amount {
        //                     *o.get_mut() += adjusted_amount.clone();
        //                 }
        //             }
        //             Entry::Vacant(v) => continue,
        //         }
        //
        //         // add to transfer recipient
        //         let mut inner = deltas.entry(transfer.to).or_default();
        //         apply_entry(transfer.token, adjusted_amount.clone(), &mut
        // inner);     }
        // }
    }
}

fn apply_entry<K: PartialEq + Hash + Eq>(
    token: K,
    amount: Rational,
    token_map: &mut HashMap<K, Rational>,
) {
    match token_map.entry(token) {
        Entry::Occupied(mut o) => {
            *o.get_mut() += amount;
        }
        Entry::Vacant(v) => {
            v.insert(amount);
        }
    }
}
