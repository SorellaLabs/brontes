use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::{hex, B256};
use brontes_database::{Metadata, Pair};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    normalized_actions::{Actions, NormalizedTransfer},
    ToScaledRational,
};
use malachite::{
    num::{
        basic::traits::{One, Zero},
        conversion::{string::options::ToSciOptions, traits::ToSci},
    },
    Rational,
};
use reth_primitives::Address;
use tracing::{info, log::debug};

#[derive(Debug)]
pub struct SharedInspectorUtils<'db> {
    quote: Address,
    db:    &'db Libmdbx,
}

impl<'db> SharedInspectorUtils<'db> {
    pub fn new(quote_address: Address, db: &'db Libmdbx) -> Self {
        SharedInspectorUtils { quote: quote_address, db }
    }

    pub fn try_get_decimals(&self, address: Address) -> Option<u8> {
        self.db.try_get_decimals(address)
    }
}

type SwapTokenDeltas = HashMap<Address, HashMap<Address, Rational>>;

impl SharedInspectorUtils<'_> {
    /// Calculates the swap deltas.
    /// Change to keep address level deltas
    pub(crate) fn calculate_token_deltas(&self, actions: &Vec<Vec<Actions>>) -> SwapTokenDeltas {
        let mut transfers = Vec::new();
        // Address and there token delta's
        let mut deltas = HashMap::new();

        for action in actions.into_iter().flatten() {
            // If the action is a swap, get the decimals to scale the amount in and out
            // properly.
            if let Actions::Swap(swap) = action {
                let Some(decimals_in) = self.db.try_get_decimals(swap.token_in) else {
                    debug!("token decimals not found");
                    continue;
                };
                let Some(decimals_out) = self.db.try_get_decimals(swap.token_out) else {
                    debug!("token decimals not found");
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
        deltas: SwapTokenDeltas,
        metadata: Arc<Metadata>,
        cex: bool,
    ) -> Option<HashMap<Address, Rational>> {
        let mut usd_deltas = HashMap::new();

        for (address, inner_map) in deltas {
            for (token_addr, amount) in inner_map {
                let pair = Pair(token_addr, self.quote);
                let price = if cex {
                    // Fetch CEX price
                    metadata.cex_quotes.get_binance_quote(&pair)?.best_ask()
                } else {
                    metadata.dex_quotes.price_after(pair, tx_position)?
                };

                let usd_amount = amount * price;

                *usd_deltas.entry(address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        Some(usd_deltas)
    }

    pub fn usd_delta_by_address_test(
        &self,
        tx_hash: B256,
        tx_position: usize,
        deltas: SwapTokenDeltas,
        metadata: Arc<Metadata>,
        cex: bool,
    ) -> Option<HashMap<Address, Rational>> {
        let mut usd_deltas = HashMap::new();

        for (address, inner_map) in deltas {
            for (token_addr, amount) in inner_map {
                let pair = Pair(token_addr, self.quote);
                let price = if cex {
                    // Fetch CEX price
                    metadata.cex_quotes.get_binance_quote(&pair)?.best_ask()
                } else {
                    metadata.dex_quotes.price_after(pair, tx_position)?
                };

                let usd_amount = amount * price;

                if tx_hash
                    == hex!("cccb371805f0a269bbbe778bb3325ffb09421fd8e26f1c3aa4fe204fbdbb613b")
                {
                    let mut opts = ToSciOptions::default();
                    opts.set_precision(10);

                    let amount= usd_amount.to_sci_with_options(opts).to_string();
                    let price = price.to_sci_with_options(opts).to_string();

                    info!(?token_addr, ?pair, ?amount, "usd price");
                }
                *usd_deltas.entry(address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        Some(usd_deltas)
    }

    pub fn get_dex_usd_price(
        &self,
        block_position: usize,
        token_address: Address,
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);
        metadata.dex_quotes.price_after(pair, block_position)
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
    fn transfer_deltas(&self, transfers: Vec<&NormalizedTransfer>, deltas: &mut SwapTokenDeltas) {
        for transfer in transfers.into_iter() {
            // normalize token decimals
            let Some(decimals) = self.db.try_get_decimals(transfer.token) else {
                debug!("token decimals not found");
                continue;
            };
            let adjusted_amount = transfer.amount.to_scaled_rational(decimals);

            // fill forward
            if deltas.contains_key(&transfer.from) {
                // subtract balance from sender
                let mut inner = deltas.entry(transfer.from).or_default();
                apply_entry(transfer.token, -adjusted_amount.clone(), &mut inner);

                // add to transfer recipient
                let mut inner = deltas.entry(transfer.to).or_default();
                apply_entry(transfer.token, adjusted_amount.clone(), &mut inner);
                continue
            }

            // fill backwards
            if deltas.contains_key(&transfer.to) {
                let mut inner = deltas.entry(transfer.from).or_default();
                apply_entry(transfer.token, -adjusted_amount.clone(), &mut inner);

                let mut inner = deltas.entry(transfer.to).or_default();
                apply_entry(transfer.token, adjusted_amount.clone(), &mut inner);
            }
        }
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
/*
#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use brontes_types::normalized_actions::{Actions, NormalizedSwap};
    use malachite::Integer;
    use reth_primitives::{Address, B256};

    use super::*;

    #[test]
    fn test_swap_deltas() {
        let inspector_utils = SharedInspectorUtils::new(
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
        );

        let swap1 = Actions::Swap(NormalizedSwap {
            index:      2,
            from:       Address::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            pool:       Address::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            token_out:  Address::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            amount_in:  B256::from_str(
                "0x000000000000000000000000000000000000000000000000064fbb84aac0dc8e",
            )
            .unwrap()
            .into(),
            amount_out: B256::from_str(
                "0x000000000000000000000000000000000000000000000000000065c3241b7c59",
            )
            .unwrap()
            .into(),
        });

        let swap2 = Actions::Swap(NormalizedSwap {
            index:      2,
            from:       Address::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            pool:       Address::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   Address::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            token_out:  Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            amount_in:  B256::from_str(
                "0x000000000000000000000000000000000000000000000000000065c3241b7c59",
            )
            .unwrap()
            .into(),
            amount_out: B256::from_str(
                "0x00000000000000000000000000000000000000000000000007e0871b600a7cf4",
            )
            .unwrap()
            .into(),
        });

        let swap3 = Actions::Swap(NormalizedSwap {
            index:      6,
            from:       Address::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap(),
            pool:       Address::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            token_out:  Address::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            amount_in:  B256::from_str(
                "0x0000000000000000000000000000000000000000000000000de0b6b3a7640000",
            )
            .unwrap()
            .into(),
            amount_out: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000bbcc68d833cc",
            )
            .unwrap()
            .into(),
        });

        let swaps = vec![vec![swap1, swap2, swap3]];

        let deltas = inspector_utils.calculate_swap_deltas(&swaps);

        let mut expected_map = HashMap::new();

        let mut inner_map = HashMap::new();
        inner_map.insert(
            Address::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            Rational::from(0),
        );
        inner_map.insert(
            Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            Rational::from_integers(
                Integer::from(56406919415648307u128),
                Integer::from(500000000000000000u128),
            ),
        );
        expected_map.insert(
            Address::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            inner_map,
        );

        let mut inner_map = HashMap::new();
        inner_map.insert(
            Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            Rational::from(-1),
        );
        inner_map.insert(
            Address::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            Rational::from_integers(Integer::from(51621651680499u128), Integer::from(2500000u128)),
        );
        expected_map.insert(
            Address::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap(),
            inner_map,
        );

        assert_eq!(expected_map, deltas);
    }
}
 */
