use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::Address;
use alloy_sol_types::abi::Token;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{cex::CexExchange, metadata, metadata::MetadataCombined},
    mev::{BundleHeader, MevType, TokenProfit, TokenProfits},
    normalized_actions::{Actions, NormalizedTransfer},
    pair::Pair,
    utils::ToFloatNearest,
    GasDetails, Root,
};
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};

#[derive(Debug)]
pub struct SharedInspectorUtils<'db, DB: LibmdbxReader> {
    pub(crate) quote: Address,
    #[allow(dead_code)]
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
                let adjusted_in = -(swap.amount_in.clone());
                let adjusted_out = swap.amount_out.clone();
                // we track the address deltas so we can apply transfers later on the profit
                if swap.from == swap.recipient {
                    let entry = deltas.entry(swap.from).or_insert_with(HashMap::default);
                    apply_entry(swap.token_out.address, adjusted_out, entry);
                    apply_entry(swap.token_in.address, adjusted_in, entry);
                } else {
                    let entry_recipient = deltas.entry(swap.from).or_insert_with(HashMap::default);
                    apply_entry(swap.token_in.address, adjusted_in, entry_recipient);

                    let entry_from = deltas
                        .entry(swap.recipient)
                        .or_insert_with(HashMap::default);
                    apply_entry(swap.token_out.address, adjusted_out, entry_from);
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
                    metadata.dex_quotes.price_at_or_before(pair, tx_position)?
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
        token_address: Address,
        amount: &Rational,
        metadata: &Arc<MetadataCombined>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(amount.clone())
        }

        let pair = Pair(token_address, self.quote);
        Some(
            metadata
                .dex_quotes
                .price_at_or_before(pair, block_position)?
                * amount,
        )
    }

    pub fn get_dex_usd_price(
        &self,
        block_position: usize,
        token_address: Address,
        metadata: Arc<MetadataCombined>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);
        metadata.dex_quotes.price_at_or_before(pair, block_position)
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

    pub fn build_bundle_header(
        &self,
        root: &Root<Actions>,
        metadata: Arc<MetadataCombined>,
        bundle_gas_details: &Vec<GasDetails>,
        bundle_actions: &Vec<Vec<Actions>>,
        mev_type: MevType,
        profit_usd: f64,
    ) -> BundleHeader {
        let tx_index = root.get_block_position() as u64;

        let token_profits = self.get_profit_collectors(
            tx_index,
            bundle_actions,
            metadata.clone(),
            mev_type.use_cex_pricing_for_deltas(),
        );

        BundleHeader {
            block_number: metadata.block_num,
            tx_index,
            tx_hash: root.tx_hash,
            eoa: root.head.address,
            mev_contract: root.head.data.get_to_address(),
            profit_usd,
            token_profits,
            bribe_usd: bundle_gas_details
                .iter()
                .map(|details| metadata.get_gas_price_usd(details.gas_paid()).to_float())
                .sum(),
            mev_type,
        }
    }

    pub fn get_profit_collectors(
        &self,
        tx_index: u64,
        bundle_actions: &Vec<Vec<Actions>>,
        metadata: Arc<MetadataCombined>,
        pricing: bool,
    ) -> TokenProfits {
        let deltas = self.calculate_token_deltas(bundle_actions);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, &deltas, metadata.clone(), pricing)?;

        let profit_collectors = self.profit_collectors(&addr_usd_deltas);

        self.get_token_profits(tx_index, metadata, profit_collectors, deltas, pricing)
    }

    pub fn get_token_profits(
        &self,
        tx_index: u64,
        metadata: Arc<MetadataCombined>,
        profit_collectors: Vec<Address>,
        deltas: SwapTokenDeltas,
        use_cex_pricing: bool,
    ) -> TokenProfits {
        let token_profits = profit_collectors
            .into_iter()
            .filter_map(|collector| deltas.get(&collector))
            .flat_map(|token_amounts| token_amounts.iter())
            .map(|(&token, &amount)| {
                let usd_value = if use_cex_pricing {
                    self.get_cex_usd_value(token, amount, &metadata)
                } else {
                    self.get_dex_usd_value(token, amount, tx_index, &metadata)
                };

                TokenProfit {
                    profit_collector: collector,
                    token,
                    amount: amount.to_float(),
                    usd_value: usd_value.to_float(),
                }
            })
            .collect();

        TokenProfits { profits: token_profits }
    }

    fn get_cex_usd_value(
        &self,
        token: Address,
        amount: Rational,
        metadata: &MetadataCombined,
    ) -> Rational {
        metadata
            .cex_quotes
            .get_quote(&Pair(token, self.quote), &CexExchange::Binance)
            .unwrap_or_default()
            .price
            .1
            * amount
    }

    fn get_dex_usd_value(
        &self,
        token: Address,
        amount: Rational,
        tx_index: u64,
        metadata: &MetadataCombined,
    ) -> Rational {
        metadata
            .dex_quotes
            .price_at_or_before(Pair(token, self.quote), tx_index)
            .unwrap_or(Rational::ZERO)
            * amount
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
