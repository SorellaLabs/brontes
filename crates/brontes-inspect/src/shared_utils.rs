use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{cex::CexExchange, dex::PriceAt, metadata::Metadata},
    mev::{BundleHeader, MevType, TokenProfit, TokenProfits},
    normalized_actions::Actions,
    pair::Pair,
    utils::ToFloatNearest,
    GasDetails, TxInfo,
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
        // Address and there token delta's
        let mut deltas = HashMap::new();

        for action in actions.into_iter().flatten() {
            if !action.is_swap() {
                continue
            }

            let swap = action.force_swap_ref();
            let amount_in = -swap.amount_in.clone();
            let amount_out = swap.amount_out.clone();
            // we track the address deltas so we can apply transfers later on the profit
            if swap.from == swap.recipient {
                let entry = deltas.entry(swap.from).or_insert_with(HashMap::default);
                apply_entry(swap.token_out.address, amount_out, entry);
                apply_entry(swap.token_in.address, amount_in, entry);
            } else {
                let entry_recipient = deltas.entry(swap.from).or_insert_with(HashMap::default);
                apply_entry(swap.token_in.address, amount_in, entry_recipient);

                let entry_from = deltas
                    .entry(swap.recipient)
                    .or_insert_with(HashMap::default);
                apply_entry(swap.token_out.address, amount_out, entry_from);
            }
        }

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
        at: PriceAt,
        deltas: &SwapTokenDeltas,
        metadata: Arc<Metadata>,
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
                        .as_ref()?
                        .price_at_or_before(pair, tx_position)
                        .map(|price| price.get_price(at))?
                        .clone()
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
        at: PriceAt,
        token_address: Address,
        amount: &Rational,
        metadata: &Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(amount.clone())
        }

        let pair = Pair(token_address, self.quote);
        Some(
            metadata
                .dex_quotes
                .as_ref()?
                .price_at_or_before(pair, block_position)?
                .get_price(at)
                * amount,
        )
    }

    pub fn get_dex_usd_price(
        &self,
        block_position: usize,
        at: PriceAt,
        token_address: Address,
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);
        metadata
            .dex_quotes
            .as_ref()?
            .price_at_or_before(pair, block_position)
            .map(|price| price.get_price(at))
    }

    pub fn profit_collectors(&self, addr_usd_deltas: &HashMap<Address, Rational>) -> Vec<Address> {
        addr_usd_deltas
            .iter()
            .filter_map(|(addr, value)| (*value > Rational::ZERO).then(|| *addr))
            .collect()
    }

    pub fn build_bundle_header(
        &self,
        info: &TxInfo,
        profit_usd: f64,
        at: PriceAt,
        actions: &Vec<Vec<Actions>>,
        gas_details: &Vec<GasDetails>,
        metadata: Arc<Metadata>,
        mev_type: MevType,
    ) -> BundleHeader {
        let token_profits = self
            .get_profit_collectors(
                info.tx_index,
                at,
                actions,
                metadata.clone(),
                mev_type.use_cex_pricing_for_deltas(),
            )
            .unwrap_or_default();
        let bribe_usd = gas_details
            .iter()
            .map(|details| metadata.get_gas_price_usd(details.gas_paid()).to_float())
            .sum::<f64>();

        BundleHeader {
            block_number: metadata.block_num,
            tx_index: info.tx_index,
            tx_hash: info.tx_hash,
            eoa: info.eoa,
            mev_contract: info.mev_contract,
            profit_usd,
            token_profits,
            bribe_usd,
            mev_type,
        }
    }

    pub fn get_dex_revenue_usd(
        &self,
        tx_index: u64,
        at: PriceAt,
        bundle_actions: &Vec<Vec<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let deltas = self.calculate_token_deltas(bundle_actions);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index as usize, at, &deltas, metadata.clone(), false)?;
        Some(
            addr_usd_deltas
                .values()
                .fold(Rational::ZERO, |acc, delta| acc + delta),
        )
    }

    pub fn get_profit_collectors(
        &self,
        tx_index: u64,
        at: PriceAt,
        bundle_actions: &Vec<Vec<Actions>>,
        metadata: Arc<Metadata>,
        pricing: bool,
    ) -> Option<TokenProfits> {
        let deltas = self.calculate_token_deltas(bundle_actions);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index as usize, at, &deltas, metadata.clone(), pricing)?;

        let profit_collectors = self.profit_collectors(&addr_usd_deltas);

        Some(self.get_token_profits(tx_index, at, metadata, profit_collectors, deltas, pricing)?)
    }

    pub fn get_token_profits(
        &self,
        tx_index: u64,
        at: PriceAt,
        metadata: Arc<Metadata>,
        profit_collectors: Vec<Address>,
        deltas: SwapTokenDeltas,
        use_cex_pricing: bool,
    ) -> Option<TokenProfits> {
        let token_profits = profit_collectors
            .into_iter()
            .filter_map(|collector| deltas.get(&collector).map(|d| (collector, d)))
            .flat_map(|(collector, token_amounts)| {
                token_amounts
                    .into_iter()
                    .zip(vec![collector].into_iter().cycle())
            })
            .filter_map(|((token, amount), collector)| {
                let usd_value = if use_cex_pricing {
                    self.get_cex_usd_value(*token, amount.clone(), &metadata)
                } else {
                    self.get_dex_usd_value(*token, at, amount.clone(), tx_index, &metadata)?
                };

                Some(TokenProfit {
                    profit_collector: collector,
                    token:            self.db.try_fetch_token_info(*token).ok()?,
                    amount:           amount.clone().to_float(),
                    usd_value:        usd_value.to_float(),
                })
            })
            .collect();

        Some(TokenProfits { profits: token_profits })
    }

    fn get_cex_usd_value(&self, token: Address, amount: Rational, metadata: &Metadata) -> Rational {
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
        at: PriceAt,
        amount: Rational,
        tx_index: u64,
        metadata: &Metadata,
    ) -> Option<Rational> {
        Some(
            metadata
                .dex_quotes
                .as_ref()?
                .price_at_or_before(Pair(token, self.quote), tx_index as usize)
                .map(|price| price.get_price(at).clone())
                .unwrap_or_else(|| {
                    tracing::error!(?token, "unwrap occured for");
                    Rational::ZERO
                })
                * amount,
        )
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
