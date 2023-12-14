use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::{hex, FixedBytes};
use alloy_providers::provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport_http::Http;
use brontes_database::{Metadata, Pair};
use brontes_types::{
    cache_decimals, normalized_actions::Actions, try_get_decimals, ToScaledRational,
    TOKEN_TO_DECIMALS,
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

type SwapTokenDeltas = HashMap<Pair, (Rational, Rational)>;
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

                if adjusted_out == Rational::ZERO || adjusted_in == Rational::ZERO {
                    error!(?swap, "amount in | amount out of the swap are zero");
                    continue
                }

                // buying token out amount.
                // Store the amount_in amount_out deltas for a given from address
                match deltas.entry(swap.from) {
                    Entry::Occupied(mut o) => {
                        let inner: &mut HashMap<Pair, (Vec<Rational>, Vec<Rational>)> = o.get_mut();

                        let pair_out = Pair(swap.token_in, swap.token_out);
                        apply_entry_with_price(
                            pair_out,
                            (&adjusted_out / -(adjusted_in.clone()), adjusted_out.clone()),
                            inner,
                        );

                        let pair_in = Pair(swap.token_out, swap.token_in);
                        apply_entry_with_price(
                            pair_in,
                            (-(adjusted_in.clone()) / &adjusted_out, adjusted_in),
                            inner,
                        );
                    }
                    Entry::Vacant(v) => {
                        let mut default = HashMap::default();

                        let pair_out = Pair(swap.token_in, swap.token_out);
                        default.insert(
                            pair_out,
                            (
                                vec![&adjusted_out / -(adjusted_in.clone())],
                                vec![adjusted_out.clone()],
                            ),
                        );

                        let pair_in = Pair(swap.token_out, swap.token_in);
                        default.insert(
                            pair_in,
                            (vec![-(adjusted_in.clone()) / &adjusted_out], vec![adjusted_in]),
                        );

                        v.insert(default);
                    }
                }

            // If there is a transfer, push to the given transfer addresses.
            } else if let Actions::Transfer(transfer) = action {
                transfers.push(transfer);
            }
        }

        // Now that all swap deltas have been calculated for a given from address we
        // need to apply all transfers that occurred. This is to move all the
        // funds to there end account to ensure for a given address what the
        // exact delta's are.
        let mut token_collectors = HashMap::new();
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
                if deltas.contains_key(&transfer.from)
                    || token_collectors.contains_key(&transfer.from)
                {
                    changed = true;

                    let mut inner = token_collectors.entry(transfer.from).or_default();
                    apply_entry(transfer.token, -adjusted_amount.clone(), &mut inner);
                } else {
                    reuse.push(transfer);
                    continue
                }

                // add value to the destination address
                let to_token_map = token_collectors.entry(transfer.to).or_default();
                apply_entry(transfer.token, adjusted_amount, to_token_map);
            }

            transfers = reuse;

            if changed == false {
                break
            }
        }

        let mut deltas: HashMap<Pair, (Rational, Rational)> = deltas
            .into_values()
            .map(|v| v.into_iter())
            .fold(HashMap::new(), |mut a, b| {
                for (k, (ratio, am)) in b {
                    let weight =
                        am.iter()
                            .map(|i| {
                                if i.lt(&Rational::ZERO) {
                                    i * Rational::from(-1)
                                } else {
                                    i.clone()
                                }
                            })
                            .sum::<Rational>();

                    let weighted_price = ratio
                        .iter()
                        .zip(am.iter())
                        .map(|(r, i)| {
                            if i.lt(&Rational::ZERO) {
                                (r, i * Rational::from(-1))
                            } else {
                                (r, i.clone())
                            }
                        })
                        .map(|(ratio, am)| ratio * am)
                        .sum::<Rational>()
                        / weight;

                    // fetch weighted,
                    *a.entry(k).or_default() = (weighted_price, am.into_iter().sum::<Rational>());
                }
                a
            });

        deltas.retain(|k, v| (v.1).ne(&Rational::ZERO));

        let token_collectors = token_collectors
            .into_iter()
            .filter(|(addr, inner)| !inner.values().any(|f| f.eq(&Rational::ZERO)))
            .map(|(addr, _)| addr)
            .collect::<Vec<_>>();

        (deltas, token_collectors)
    }

    pub fn get_usd_price(&self, token: Address, metadata: Arc<Metadata>) -> Option<Rational> {
        if token == self.0 {
            return Some(Rational::ONE)
        }

        let pair = Pair(token, self.0);
        metadata.cex_quotes.get_quote(&pair).map(|v| v.avg())
    }

    /// applies usd price to deltas and flattens out the tokens
    pub fn usd_delta(
        &self,
        deltas: HashMap<Pair, (Rational, Rational)>,
        metadata: Arc<Metadata>,
    ) -> Rational {
        deltas
            .into_iter()
            .filter_map(|(pair, (dex_price, value))| {
                // if the pair has a edge with our base, then just use the given price
                if pair.0 == self.0 {
                    info!(?pair, ?dex_price, ?value, "pair.0 == self.0");
                    return Some(dex_price * value)
                }
                if pair.1 == self.0 {
                    info!(?pair, ?dex_price, ?value, "pair.1 == self.0");
                    let (num, denom) = dex_price.into_numerator_and_denominator();
                    return Some(Rational::from_naturals(denom, num) * value)
                }

                let search_pair_0 = Pair(pair.1, self.0);
                let search_pair_1 = Pair(pair.0, self.0);

                if let Some(res) = metadata.cex_quotes.get_quote(&search_pair_0) {
                    Some(value * res.avg())
                } else if let Some(res) = metadata.cex_quotes.get_quote(&search_pair_1) {
                    let price = res.avg() / dex_price;
                    Some(value * price)
                } else {
                    error!(?pair, "was unable to find a price");
                    return None
                }
            })
            .sum::<Rational>()
    }
}

fn apply_entry_with_price<K: PartialEq + Hash + Eq>(
    token: K,
    amount: (Rational, Rational),
    token_map: &mut HashMap<K, (Vec<Rational>, Vec<Rational>)>,
) {
    match token_map.entry(token) {
        Entry::Occupied(mut o) => {
            let (dex_price, am) = o.get_mut();
            dex_price.push(amount.0);
            am.push(amount.1);
        }
        Entry::Vacant(v) => {
            v.insert((vec![amount.0], vec![amount.1]));
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
