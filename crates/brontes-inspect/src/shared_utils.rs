use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::FixedBytes;
use alloy_providers::provider::Provider;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport_http::Http;
use brontes_database::Metadata;
use brontes_types::{
    cache_decimals, normalized_actions::Actions, try_get_decimals, ToScaledRational,
    TOKEN_TO_DECIMALS,
};
use futures::stream::StreamExt;
use malachite::{num::basic::traits::Zero, Rational};
use reth_primitives::Address;
use reth_rpc_types::TransactionRequest;
use tracing::{error, warn};

sol!(
    function decimals() public view returns (uint8);
);

#[derive(Debug)]
pub struct SharedInspectorUtils {
    // will update to direct db read later
    provider: Provider<Http<reqwest::Client>>,
}

impl SharedInspectorUtils {
    pub fn new(url: &String) -> Self {
        Self { provider: Provider::new(url).unwrap() }
    }

    pub async fn get_decimals(&self, addr: [u8; 20]) -> Option<u8> {
        if let Some(decimals) = try_get_decimals(&addr) {
            Some(decimals)
        } else {
            // query this
            let call = decimalsCall::new(()).abi_encode();
            let mut tx_req = TransactionRequest::default()
                .to(Address(FixedBytes(addr.clone())))
                .input(call);

            if let Some(res) = self.provider.call(tx_req, None).await.ok() {
                let Some(dec) = decimalsCall::abi_decode_returns(&res, true).ok() else {
                    return None
                };
                let dec = dec._0;
                cache_decimals(addr, dec);
                //TODO: insert into db

                return Some(dec)
            } else {
                warn!("Token request failed for token {:?}", addr);
            }

            None
        }
    }

    /// Calculates the swap deltas. if transfers are also passed in. we also
    /// move those deltas on the map around accordingly.
    /// NOTE: the upper level inspector needs to know if the transfer is related
    /// to the underlying swap. action otherwise you could get misreads
    pub(crate) async fn calculate_swap_deltas(
        &self,
        actions: &Vec<Vec<Actions>>,
    ) -> HashMap<Address, HashMap<Address, Rational>> {
        let mut transfers = Vec::new();

        // Address and there token delta's
        let mut deltas = HashMap::new();

        for action in actions.into_iter().flatten() {
            // If the action is a swap, get the decimals to scale the amount in and out
            // properly.
            if let Actions::Swap(swap) = action {
                let Some(decimals_in) = self.get_decimals(swap.token_in.0 .0).await else {
                    continue;
                };
                let Some(decimals_out) = self.get_decimals(swap.token_out.0 .0).await else {
                    continue;
                };

                let adjusted_in = -swap.amount_in.to_scaled_rational(decimals_in);
                let adjusted_out = swap.amount_out.to_scaled_rational(decimals_out);

                // Store the amount_in amount_out deltas for a given from address
                match deltas.entry(swap.from) {
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
            // If there is a transfer, push to the given transfer addresses.
            } else if let Actions::Transfer(transfer) = action {
                transfers.push(transfer);
            }
        }

        // Now that all swap deltas have been calculated for a given from address we
        // need to apply all transfers that occurred. This is to move all the
        // funds to there end account to ensure for a given address what the
        // exact delta's are.
        loop {
            let mut changed = false;
            let mut reuse = Vec::new();

            for transfer in transfers.into_iter() {
                // normalize token decimals
                let decimals = self.get_decimals(transfer.token.0 .0).await?;

                let adjusted_amount = transfer.amount.to_scaled_rational(decimals);

                // subtract value from the from address
                if let Some(from_token_map) = deltas.get_mut(&transfer.from) {
                    changed = true;
                    apply_entry(transfer.token, -adjusted_amount.clone(), from_token_map);
                } else {
                    reuse.push(transfer)
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
    }

    /// applies usd price to deltas and flattens out the tokens
    pub(crate) fn get_best_usd_deltas(
        &self,
        deltas: HashMap<Address, HashMap<Address, Rational>>,
        metadata: Arc<Metadata>,
        time_selector: Box<dyn Fn(&(Rational, Rational)) -> &Rational>,
    ) -> HashMap<Address, Rational> {
        deltas
            .into_iter()
            .map(|(caller, tokens)| {
                let summed_value = tokens
                    .into_iter()
                    .map(|(address, mut value)| {
                        if let Some(price) = metadata.token_prices.get(&address) {
                            value *= time_selector(price);
                            value
                        } else {
                            Rational::ZERO
                        }
                    })
                    .sum::<Rational>();
                (caller, summed_value)
            })
            .collect()
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
