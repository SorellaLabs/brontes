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
    normalized_actions::{NormalizedSwap, NormalizedTransfer},
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
type TokenDeltas = HashMap<Address, Rational>;
type AddressDeltas = HashMap<Address, TokenDeltas>;

impl<DB: LibmdbxReader> SharedInspectorUtils<'_, DB> {
    /// Calculates the token balance deltas by address for a given set of swaps
    /// Note this does not account for the pool delta's, only the swapper and
    /// recipient delta's
    pub(crate) fn calculate_swap_deltas(&self, swaps: &[NormalizedSwap]) -> AddressDeltas {
        // Address and there token delta's
        let mut deltas: AddressDeltas = HashMap::new();
        swaps
            .iter()
            .for_each(|swap| swap.apply_token_deltas(&mut deltas));

        deltas
    }

    /// Calculates the token balance deltas by address for a given set of
    /// transfers
    pub fn calculate_transfer_deltas(&self, transfers: &[NormalizedTransfer]) -> AddressDeltas {
        let mut deltas: AddressDeltas = HashMap::new();
        transfers
            .iter()
            .for_each(|transfer| transfer.apply_token_deltas(&mut deltas));

        deltas
    }

    /// Calculates the USD value of the token balance deltas by address
    pub fn usd_delta_by_address(
        &self,
        tx_position: u64,
        at: PriceAt,
        deltas: &AddressDeltas,
        metadata: Arc<Metadata>,
        cex: bool,
    ) -> Option<HashMap<Address, Rational>> {
        let mut usd_deltas = HashMap::new();

        for (address, token_deltas) in deltas {
            for (token_addr, amount) in token_deltas {
                let pair = Pair(*token_addr, self.quote);
                let price = if cex {
                    metadata.cex_quotes.get_binance_quote(&pair)?.best_ask()
                } else {
                    metadata
                        .dex_quotes
                        .as_ref()?
                        .price_at_or_before(pair, tx_position as usize)
                        .map(|price| price.get_price(at))?
                        .clone()
                };

                let usd_amount = amount.clone() * price.clone();

                *usd_deltas.entry(*address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        Some(usd_deltas)
    }

    pub fn get_token_value_dex(
        &self,
        tx_index: usize,
        at: PriceAt,
        token_address: Address,
        amount: &Rational,
        metadata: &Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(amount.clone());
        }
        let price = self.get_token_price_on_dex(tx_index, at, token_address, metadata)?;
        Some(price * amount)
    }

    pub fn get_token_price_on_dex(
        &self,
        tx_index: usize,
        at: PriceAt,
        token_address: Address,
        metadata: &Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE);
        }

        let pair = Pair(token_address, self.quote);

        Some(
            metadata
                .dex_quotes
                .as_ref()?
                .price_at_or_before(pair, tx_index)?
                .get_price(at),
        )
    }

    fn get_token_value_cex(
        &self,
        token: Address,
        amount: Rational,
        metadata: &Metadata,
    ) -> Rational {
        metadata
            .cex_quotes
            .get_quote(&Pair(token, self.quote), &CexExchange::Binance)
            .unwrap_or_default()
            .price
            .1
            * amount
    }

    pub fn profit_collectors(&self, addr_usd_deltas: &HashMap<Address, Rational>) -> Vec<Address> {
        addr_usd_deltas
            .iter()
            .filter(|&(_, value)| *value > Rational::ZERO)
            .map(|(&addr, _)| addr)
            .collect()
    }

    pub fn build_bundle_header(
        &self,
        info: &TxInfo,
        profit_usd: f64,
        at: PriceAt,
        bundle_transfers: Vec<&[NormalizedTransfer]>,
        gas_details: &[GasDetails],
        metadata: Arc<Metadata>,
        mev_type: MevType,
    ) -> BundleHeader {
        //TODO: Figure out how we can calculate the token profits for this bundle using
        // generic actions
        let token_profits = self
            .get_profit_collectors(
                info.tx_index,
                at,
                bundle_transfers,
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

    pub fn get_swap_deltas_usd(
        &self,
        tx_index: u64,
        at: PriceAt,
        swaps: &[NormalizedSwap],
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let deltas = self.calculate_swap_deltas(swaps);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, at, &deltas, metadata.clone(), false)?;
        Some(
            addr_usd_deltas
                .values()
                .fold(Rational::ZERO, |acc, delta| acc + delta),
        )
    }

    pub fn get_transfers_deltas_usd(
        &self,
        tx_index: u64,
        at: PriceAt,
        transfers: &[NormalizedTransfer],
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let deltas = self.calculate_transfer_deltas(transfers);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, at, &deltas, metadata.clone(), false)?;
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
        bundle_transfers: &[NormalizedTransfer],
        metadata: Arc<Metadata>,
        pricing: bool,
    ) -> Option<TokenProfits> {
        let deltas = self.calculate_transfer_deltas(bundle_transfers);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, at, &deltas, metadata.clone(), pricing)?;

        let profit_collectors = self.profit_collectors(&addr_usd_deltas);

        self.get_token_profits(tx_index as usize, at, metadata, profit_collectors, deltas, pricing)
    }

    pub fn get_token_profits(
        &self,
        tx_index: usize,
        at: PriceAt,
        metadata: Arc<Metadata>,
        profit_collectors: Vec<Address>,
        deltas: AddressDeltas,
        use_cex_pricing: bool,
    ) -> Option<TokenProfits> {
        let token_profits = profit_collectors
            .into_iter()
            .filter_map(|collector| deltas.get(&collector).map(|d| (collector, d)))
            .flat_map(|(collector, token_amounts)| {
                token_amounts
                    .iter()
                    .zip(vec![collector].into_iter().cycle())
            })
            .filter_map(|((token, amount), collector)| {
                let usd_value = if use_cex_pricing {
                    self.get_token_value_cex(*token, amount.clone(), &metadata)
                } else {
                    self.get_token_value_dex(tx_index, at, *token, amount, &metadata)?
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
}

pub trait TokenAccounting {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas);
}

impl TokenAccounting for NormalizedSwap {
    /// Note that we skip the pool deltas accounting to focus solely on the
    /// swapper & recipients delta. We might want to change this in the
    /// future.
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_in = -self.amount_in.clone();
        let amount_out = self.amount_out.clone();

        apply_delta(self.from, self.token_in.address, amount_in, delta_map);
        apply_delta(self.recipient, self.token_out.address, amount_out, delta_map);
    }
}

impl TokenAccounting for NormalizedTransfer {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_sent = &self.amount + &self.fee;

        apply_delta(self.from, self.token.address, -amount_sent.clone(), delta_map);

        apply_delta(self.to, self.token.address, self.amount.clone(), delta_map);
    }
}

fn apply_delta<K: PartialEq + Hash + Eq>(
    address: K,
    token: K,
    amount: Rational,
    delta_map: &mut HashMap<K, HashMap<K, Rational>>,
) {
    match delta_map.entry(address).or_default().entry(token) {
        Entry::Occupied(mut o) => {
            *o.get_mut() += amount;
        }
        Entry::Vacant(v) => {
            v.insert(amount);
        }
    }
}
