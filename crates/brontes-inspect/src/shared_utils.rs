use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::{hex, FixedBytes, B256};
use alloy_providers::provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport_http::Http;
use brontes_database::{Metadata, Pair};
use brontes_types::{
    cache_decimals,
    normalized_actions::{Actions, NormalizedTransfer},
    try_get_decimals, ToScaledRational, TOKEN_TO_DECIMALS,
};
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use reth_primitives::Address;
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct SharedInspectorUtils(Address);

impl SharedInspectorUtils {
    pub fn new(quote_address: Address) -> SharedInspectorUtils {
        SharedInspectorUtils(quote_address)
    }
}

type SwapTokenDeltas = HashMap<Address, Rational>;
type TokenCollectors = Vec<Address>;

impl SharedInspectorUtils {
    /// Calculates the swap deltas.
    pub(crate) fn calculate_swap_deltas(
        &self,
        actions: &Vec<Vec<Actions>>,
    ) -> (SwapTokenDeltas, TokenCollectors) {
        let mut transfers = Vec::new();
        // Address and there token delta's
        let mut deltas = HashMap::new();

        for action in actions.into_iter().flatten() {
            // If the action is a swap, get the decimals to scale the amount in and out
            // properly.
            if let Actions::Swap(swap) = action {
                let Some(decimals_in) = try_get_decimals(&swap.token_in.0 .0) else {
                    continue;
                };
                let Some(decimals_out) = try_get_decimals(&swap.token_out.0 .0) else {
                    continue;
                };

                let adjusted_in = -swap.amount_in.to_scaled_rational(decimals_in);
                let adjusted_out = swap.amount_out.to_scaled_rational(decimals_out);

                // we track from so we can apply transfers later on the profit collector
                match deltas.entry(swap.from) {
                    Entry::Occupied(mut o) => {
                        let inner: &mut HashMap<Address, Rational> = o.get_mut();

                        apply_entry(swap.token_out, adjusted_out, inner);
                        apply_entry(swap.token_in, adjusted_in, inner);
                    }
                    Entry::Vacant(v) => {
                        let mut default = HashMap::default();

                        default.insert(swap.token_out, adjusted_out);
                        default.insert(swap.token_in, adjusted_in);

                        v.insert(default);
                    }
                }

            // If there is a transfer, push to the given transfer addresses.
            } else if let Actions::Transfer(transfer) = action {
                transfers.push(transfer);
            }
        }

        let token_collectors = self.token_collectors(transfers, &mut deltas);

        // drop all zero value tokens
        let deltas = deltas
            .into_iter()
            .map(|(_, mut v)| {
                v.retain(|k, rational| (*rational).ne(&Rational::ZERO));
                v
            })
            .fold(HashMap::new(), |mut map, inner| {
                for (k, v) in inner {
                    *map.entry(k).or_default() += v;
                }
                map
            });

        (deltas, token_collectors)
    }

    pub fn get_usd_price_dex_avg(
        &self,
        prev_tx: &B256,
        curr_tx: &B256,
        token_address: Address,
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.0 {
            return Some(Rational::ONE)
        }
        let pair = Pair(token_address, self.0);

        metadata
            .dex_quotes
            .get_quote(&pair)
            .map(|q| (q.get_price(prev_tx) + q.get_price(curr_tx)) / Rational::from(2))
    }

    /// applies usd price to deltas and flattens out the tokens
    pub fn usd_delta_dex_avg(
        &self,
        prev_tx: &B256,
        current_tx: &B256,
        deltas: HashMap<Address, Rational>,
        metadata: Arc<Metadata>,
    ) -> Rational {
        deltas
            .into_iter()
            .filter_map(|(token_out, value)| {
                let pair = Pair(token_out, self.0);
                metadata
                    .dex_quotes
                    .get_quote(&pair)
                    .map(|q| {
                        Some((q.get_price(prev_tx) + q.get_price(current_tx)) / Rational::from(2))
                    })
                    .unwrap_or_else(|| {
                        error!(?pair, "was unable to find a price");
                        None
                    })
            })
            .sum::<Rational>()
    }

    fn token_collectors(
        &self,
        mut transfers: Vec<&NormalizedTransfer>,
        deltas: &mut HashMap<Address, HashMap<Address, Rational>>,
    ) -> Vec<Address> {
        loop {
            let mut changed = false;
            let mut reuse = Vec::new();

            for transfer in transfers.into_iter() {
                // normalize token decimals
                let Some(decimals) = try_get_decimals(&transfer.token.0 .0) else {
                    continue;
                };

                let adjusted_amount = transfer.amount.to_scaled_rational(decimals);

                // if deltas has the entry or token_collector does, then we move it
                if deltas.contains_key(&transfer.from) {
                    changed = true;
                    let mut inner = deltas.entry(transfer.from).or_default();
                    apply_entry(transfer.token, -adjusted_amount.clone(), &mut inner);
                } else {
                    reuse.push(transfer);
                    continue
                }
                // add value to the destination address
                let to_token_map = deltas.entry(transfer.to).or_default();
                apply_entry(transfer.token, adjusted_amount, to_token_map);
            }
            transfers = reuse;

            if changed == false {
                break
            }
        }

        deltas
            .iter()
            .filter(|(addr, inner)| !inner.values().all(|f| f.eq(&Rational::ZERO)))
            .map(|(addr, _)| *addr)
            .collect::<Vec<_>>()
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use brontes_types::normalized_actions::{Actions, NormalizedSwap};
    use malachite::Integer;
    use reth_primitives::{H160, H256};

    use super::*;

    #[test]
    fn test_swap_deltas() {
        let inspector_utils = SharedInspectorUtils::default();

        let swap1 = Actions::Swap(NormalizedSwap {
            index:      2,
            from:       H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            pool:       H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            token_out:  H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            amount_in:  H256::from_str(
                "0x000000000000000000000000000000000000000000000000064fbb84aac0dc8e",
            )
            .unwrap()
            .into(),
            amount_out: H256::from_str(
                "0x000000000000000000000000000000000000000000000000000065c3241b7c59",
            )
            .unwrap()
            .into(),
        });

        let swap2 = Actions::Swap(NormalizedSwap {
            index:      2,
            from:       H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            pool:       H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            token_out:  H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            amount_in:  H256::from_str(
                "0x000000000000000000000000000000000000000000000000000065c3241b7c59",
            )
            .unwrap()
            .into(),
            amount_out: H256::from_str(
                "0x00000000000000000000000000000000000000000000000007e0871b600a7cf4",
            )
            .unwrap()
            .into(),
        });

        let swap3 = Actions::Swap(NormalizedSwap {
            index:      6,
            from:       H160::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap(),
            pool:       H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(),
            token_in:   H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            token_out:  H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            amount_in:  H256::from_str(
                "0x0000000000000000000000000000000000000000000000000de0b6b3a7640000",
            )
            .unwrap()
            .into(),
            amount_out: H256::from_str(
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
            H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            Rational::from(0),
        );
        inner_map.insert(
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            Rational::from_integers(
                Integer::from(56406919415648307u128),
                Integer::from(500000000000000000u128),
            ),
        );
        expected_map.insert(
            H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            inner_map,
        );

        let mut inner_map = HashMap::new();
        inner_map.insert(
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            Rational::from(-1),
        );
        inner_map.insert(
            H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
            Rational::from_integers(Integer::from(51621651680499u128), Integer::from(2500000u128)),
        );
        expected_map.insert(
            H160::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap(),
            inner_map,
        );

        assert_eq!(expected_map, deltas);
    }
}
