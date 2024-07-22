use std::sync::Arc;

use alloy_primitives::{Address, FixedBytes};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_metrics::inspectors::OutlierMetrics;
use brontes_types::{
    db::{
        dex::{BlockPrice, PriceAt},
        metadata::Metadata,
        token_info::TokenInfoWithAddress,
    },
    mev::{AddressBalanceDeltas, BundleHeader, MevType, TokenBalanceDelta, TransactionAccounting},
    normalized_actions::{
        Action, NormalizedAggregator, NormalizedBatch, NormalizedFlashLoan, NormalizedSwap,
        NormalizedTransfer,
    },
    pair::Pair,
    utils::ToFloatNearest,
    ActionIter, FastHashMap, FastHashSet, GasDetails, TxInfo,
};
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use reth_primitives::TxHash;

#[derive(Debug)]
pub struct SharedInspectorUtils<'db, DB: LibmdbxReader> {
    pub(crate) quote: Address,
    pub(crate) db:    &'db DB,
    pub metrics:      Option<OutlierMetrics>,
}

impl<'db, DB: LibmdbxReader> SharedInspectorUtils<'db, DB> {
    pub fn new(quote_address: Address, db: &'db DB, metrics: Option<OutlierMetrics>) -> Self {
        SharedInspectorUtils { quote: quote_address, db, metrics }
    }
}
type TokenDeltas = FastHashMap<Address, Rational>;
type AddressDeltas = FastHashMap<Address, TokenDeltas>;

impl<DB: LibmdbxReader> SharedInspectorUtils<'_, DB> {
    pub fn get_metrics(&self) -> Option<&OutlierMetrics> {
        self.metrics.as_ref()
    }

    /// Calculates the USD value of the token balance deltas by address
    pub fn usd_delta_by_address(
        &self,
        tx_position: u64,
        at: PriceAt,
        deltas: &AddressDeltas,
        metadata: Arc<Metadata>,
        cex: bool,
        at_or_before: bool,
    ) -> Option<FastHashMap<Address, Rational>> {
        let mut usd_deltas = FastHashMap::default();

        for (address, token_deltas) in deltas {
            for (token_addr, amount) in token_deltas {
                if amount == &Rational::ZERO {
                    continue
                }

                let pair = Pair(*token_addr, self.quote);
                let price = if cex {
                    metadata.cex_quotes.get_binance_quote(&pair)?.price_maker.1
                } else if at_or_before {
                    metadata
                        .dex_quotes
                        .as_ref()?
                        .price_at_or_before(pair, tx_position as usize)
                        .map(|price| price.get_price(at))?
                        .clone()
                } else {
                    metadata
                        .dex_quotes
                        .as_ref()?
                        .price_at(pair, tx_position as usize)
                        .map(|price| price.get_price(at))?
                        .clone()
                };

                let usd_amount = amount.clone() * price.clone();

                *usd_deltas.entry(*address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        Some(usd_deltas)
    }

    // will flatten nested and filter out actions that aren't swap, transfer or
    // eth_transfer
    pub fn flatten_nested_actions_default<'a>(
        &self,
        iter: impl Iterator<Item = Action> + 'a,
    ) -> impl Iterator<Item = Action> + 'a {
        self.flatten_nested_actions(iter, &|action| {
            action.is_swap() || action.is_transfer() || action.is_eth_transfer()
        })
    }

    pub fn flatten_nested_actions<'a, F>(
        &self,
        iter: impl Iterator<Item = Action> + 'a,
        filter_actions: &'a F,
    ) -> impl Iterator<Item = Action> + 'a
    where
        F: for<'b> Fn(&'b Action) -> bool + 'a,
    {
        iter.flatten_specified(Action::try_aggregator_ref, move |actions: NormalizedAggregator| {
            actions
                .child_actions
                .into_iter()
                .filter(&filter_actions)
                .collect::<Vec<_>>()
        })
        .flatten_specified(Action::try_flash_loan_ref, move |action: NormalizedFlashLoan| {
            action
                .fetch_underlying_actions()
                .filter(&filter_actions)
                .collect::<Vec<_>>()
        })
        .flatten_specified(Action::try_batch_ref, move |action: NormalizedBatch| {
            action
                .fetch_underlying_actions()
                .filter(&filter_actions)
                .collect::<Vec<_>>()
        })
    }

    /// defaults to zero for price if doesn't exist
    pub fn get_available_usd_deltas(
        &self,
        tx_index: u64,
        at: PriceAt,
        mev_addresses: &FastHashSet<Address>,
        deltas: &AddressDeltas,
        metadata: Arc<Metadata>,
    ) -> Rational {
        let mut usd_deltas = FastHashMap::default();

        for (address, token_deltas) in deltas {
            for (token_addr, amount) in token_deltas {
                if amount == &Rational::ZERO {
                    continue
                }

                let pair = Pair(*token_addr, self.quote);
                let price = metadata
                    .dex_quotes
                    .as_ref()
                    .and_then(|dq| {
                        dq.price_at(pair, tx_index as usize)
                            .map(|price| price.get_price(at))
                    })
                    .unwrap_or_default();

                let usd_amount = amount.clone() * price;

                *usd_deltas.entry(*address).or_insert(Rational::ZERO) += usd_amount;
            }
        }

        let sum = usd_deltas
            .iter()
            .filter_map(|(address, delta)| mev_addresses.contains(address).then_some(delta))
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        sum
    }

    /// tries to convert transfer over to swaps
    pub fn try_create_swaps(
        &self,
        transfers: &[NormalizedTransfer],
        invalid_addresses: FastHashSet<Address>,
    ) -> Vec<NormalizedSwap> {
        let mut pools: FastHashMap<Address, Vec<(TokenInfoWithAddress, bool, Rational, Address)>> =
            FastHashMap::default();

        for t in transfers {
            // we do this so if the transfer is from a mev contract or a searcher, it gets
            // ignored
            if invalid_addresses.contains(&t.from) {
                continue
            }

            pools
                .entry(t.to)
                .or_default()
                .push((t.token.clone(), true, t.amount.clone(), t.from));

            pools
                .entry(t.from)
                .or_default()
                .push((t.token.clone(), false, t.amount.clone(), t.to));
        }

        pools
            .into_iter()
            .filter_map(|(pool, mut possible_swaps)| {
                if possible_swaps.len() != 2 {
                    return None
                }

                let (f_token, f_direction, f_am, f_addr) = possible_swaps.pop()?;
                let (s_token, s_direction, s_am, s_addr) = possible_swaps.pop()?;

                if s_token == f_token || s_direction == f_direction {
                    return None
                }

                let (amount_in, amount_out, token_in, token_out, from, recip) = if f_direction {
                    (f_am, s_am, f_token, s_token, f_addr, s_addr)
                } else {
                    (s_am, f_am, s_token, f_token, s_addr, f_addr)
                };

                Some(NormalizedSwap {
                    pool,
                    amount_in,
                    amount_out,
                    token_out,
                    token_in,
                    from,
                    recipient: recip,
                    ..Default::default()
                })
            })
            .collect()
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
            return Some(amount.clone())
        }
        let price = self.get_token_price_on_dex(tx_index, at, token_address, metadata)?;
        Some(price * amount)
    }

    pub fn get_token_value_dex_block(
        &self,
        block_price: BlockPrice,
        token_address: Address,
        amount: &Rational,
        metadata: &Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(amount.clone())
        }
        let price = self.get_token_price_on_dex_block(block_price, token_address, metadata)?;
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
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);

        Some(
            metadata
                .dex_quotes
                .as_ref()?
                .price_at(pair, tx_index)?
                .get_price(at),
        )
    }

    pub fn get_token_price_on_dex_block(
        &self,
        block: BlockPrice,
        token_address: Address,
        metadata: &Arc<Metadata>,
    ) -> Option<Rational> {
        if token_address == self.quote {
            return Some(Rational::ONE)
        }

        let pair = Pair(token_address, self.quote);

        metadata.dex_quotes.as_ref()?.price_for_block(pair, block)
    }

    pub fn build_bundle_header_searcher_activity(
        &self,
        bundle_deltas: Vec<AddressDeltas>,
        bundle_txes: Vec<TxHash>,
        info: &TxInfo,
        mut profit_usd: f64,
        price_type: BlockPrice,
        gas_details: &[GasDetails],
        metadata: Arc<Metadata>,
        mev_type: MevType,
        no_pricing_calculated: bool,
    ) -> BundleHeader {
        if no_pricing_calculated {
            profit_usd = 0.0;
        }

        let balance_deltas =
            self.get_bundle_accounting(bundle_txes, bundle_deltas, |this, token, amount| {
                this.get_token_value_dex_block(price_type, token, &amount, &metadata)
            });

        let bribe_usd = gas_details
            .iter()
            .map(|details| {
                metadata
                    .get_gas_price_usd(details.gas_paid(), self.quote)
                    .to_float()
            })
            .sum::<f64>();

        let fund = info
            .get_searcher_contract_info()
            .map(|i| i.fund)
            .or_else(|| info.get_searcher_eao_info().map(|f| f.fund))
            .unwrap_or_default();

        BundleHeader {
            block_number: metadata.block_num,
            tx_index: info.tx_index,
            tx_hash: info.tx_hash,
            eoa: info.eoa,
            fund,
            mev_contract: info.mev_contract,
            profit_usd,
            bribe_usd,
            mev_type,
            no_pricing_calculated,
            balance_deltas,
        }
    }

    pub fn build_bundle_header(
        &self,
        bundle_deltas: Vec<AddressDeltas>,
        bundle_txes: Vec<TxHash>,
        info: &TxInfo,
        mut profit_usd: f64,
        gas_details: &[GasDetails],
        metadata: Arc<Metadata>,
        mev_type: MevType,
        no_pricing_calculated: bool,
        price_f: impl Fn(&Self, Address, Rational) -> Option<Rational>,
    ) -> BundleHeader {
        if no_pricing_calculated {
            profit_usd = 0.0;
        }

        let balance_deltas = self.get_bundle_accounting(bundle_txes, bundle_deltas, price_f);

        let bribe_usd = gas_details
            .iter()
            .map(|details| {
                metadata
                    .get_gas_price_usd(details.gas_paid(), self.quote)
                    .to_float()
            })
            .sum::<f64>();

        if profit_usd > bribe_usd * 100.0 {
            self.metrics
                .as_ref()
                .inspect(|m| m.inspector_100x_profit(mev_type));
        }

        let fund = info
            .get_searcher_contract_info()
            .map(|i| i.fund)
            .or_else(|| info.get_searcher_eao_info().map(|f| f.fund))
            .unwrap_or_default();

        BundleHeader {
            block_number: metadata.block_num,
            tx_index: info.tx_index,
            tx_hash: info.tx_hash,
            fund,
            eoa: info.eoa,
            mev_contract: info.mev_contract,
            profit_usd,
            bribe_usd,
            mev_type,
            no_pricing_calculated,
            balance_deltas,
        }
    }

    pub fn get_full_block_price(
        &self,
        price_type: BlockPrice,
        mev_addresses: FastHashSet<Address>,
        deltas: &AddressDeltas,
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let mut usd_deltas = FastHashMap::default();

        for (address, token_deltas) in deltas {
            for (token_addr, amount) in token_deltas {
                let pair = Pair(*token_addr, self.quote);
                let price = metadata
                    .dex_quotes
                    .as_ref()?
                    .price_for_block(pair, price_type)?;

                let usd_amount = amount.clone() * price.clone();

                *usd_deltas.entry(*address).or_insert(Rational::ZERO) += usd_amount;
            }
        }
        let sum = usd_deltas
            .iter()
            .filter_map(|(address, delta)| mev_addresses.contains(address).then_some(delta))
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        Some(sum)
    }

    pub fn get_deltas_usd(
        &self,
        tx_index: u64,
        at: PriceAt,
        mev_addresses: &FastHashSet<Address>,
        deltas: &AddressDeltas,
        metadata: Arc<Metadata>,
        at_or_before: bool,
    ) -> Option<Rational> {
        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, at, deltas, metadata.clone(), false, at_or_before)?;

        let sum = addr_usd_deltas
            .iter()
            .filter_map(|(address, delta)| mev_addresses.contains(address).then_some(delta))
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        Some(sum)
    }

    pub fn get_bundle_accounting(
        &self,
        bundle_txes: Vec<FixedBytes<32>>,
        bundle_deltas: Vec<AddressDeltas>,
        price_f: impl Fn(&Self, Address, Rational) -> Option<Rational>,
    ) -> Vec<TransactionAccounting> {
        bundle_txes
            .into_iter()
            .zip(bundle_deltas)
            .map(|(tx_hash, deltas)| {
                let address_deltas: Vec<AddressBalanceDeltas> = deltas
                    .into_iter()
                    .map(|(address, token_deltas)| {
                        let deltas: Vec<TokenBalanceDelta> = token_deltas
                            .into_iter()
                            .map(|(token, amount)| {
                                //TODO: For cex-dex if we merge swap we won't have the intermediary
                                //TODO: price so it will be marked as zero in the deltas
                                let usd_value =
                                    price_f(self, token, amount.clone()).unwrap_or(Rational::ZERO);
                                TokenBalanceDelta {
                                    token:     self
                                        .db
                                        .try_fetch_token_info(token)
                                        .ok()
                                        .unwrap_or_default(),
                                    amount:    amount.to_float(),
                                    usd_value: usd_value.to_float(),
                                }
                            })
                            .collect();

                        let name = self.fetch_address_name(address);

                        AddressBalanceDeltas { address, name, token_deltas: deltas }
                    })
                    .collect();

                TransactionAccounting { tx_hash, address_deltas }
            })
            .collect()
    }

    pub fn fetch_address_name(&self, address: Address) -> Option<String> {
        let protocol_name = self
            .db
            .get_protocol_details(address)
            .ok()
            .map(|protocol| protocol.protocol.to_string());

        protocol_name.or_else(|| {
            self.db
                .try_fetch_searcher_info(address, Some(address))
                .ok()
                .and_then(|(searcher_eoa, searcher_contract)| {
                    searcher_eoa
                        .map(|eoa| eoa.describe())
                        .or_else(|| searcher_contract.map(|contract| contract.describe()))
                })
                .or_else(|| {
                    self.db
                        .try_fetch_builder_info(address)
                        .ok()
                        .and_then(|builder_info| builder_info.map(|info| info.describe()))
                })
                .or_else(|| {
                    self.db
                        .try_fetch_address_metadata(address)
                        .ok()
                        .and_then(|metadata| metadata.map(|info| info.describe()))?
                })
        })
    }
}
