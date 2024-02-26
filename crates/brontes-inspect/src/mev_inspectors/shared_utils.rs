use core::hash::Hash;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Arc,
};

use alloy_primitives::{Address, FixedBytes};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{
    db::{cex::CexExchange, dex::PriceAt, metadata::Metadata},
    mev::{AddressBalanceDeltas, BundleHeader, MevType, TokenBalanceDelta, TransactionAccounting},
    normalized_actions::{
        Actions, NormalizedBatch, NormalizedBurn, NormalizedCollect, NormalizedFlashLoan,
        NormalizedLiquidation, NormalizedMint, NormalizedSwap, NormalizedTransfer,
    },
    pair::Pair,
    utils::ToFloatNearest,
    BlockTree, GasDetails, TreeCollect, TreeSearchBuilder, TxInfo,
};
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use reth_primitives::TxHash;

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

/* TODO: Ludwig
pub struct AddressWithContext {
    pub address:      Address,
    pub address_type: AddressType,
}

pub enum AddressType {
    Pool,
    MEV,

} */

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
            return Some(amount.clone())
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
            return Some(Rational::ONE)
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
    ) -> Option<Rational> {
        Some(
            metadata
                .cex_quotes
                .get_quote(&Pair(token, self.quote), &CexExchange::Binance)?
                .price
                .1
                * amount,
        )
    }

    pub fn build_bundle_header(
        &self,
        tree: Arc<BlockTree<Actions>>,
        bundle_txes: Vec<TxHash>,
        info: &TxInfo,
        profit_usd: f64,
        at: PriceAt,
        gas_details: &[GasDetails],
        metadata: Arc<Metadata>,
        mev_type: MevType,
    ) -> BundleHeader {
        let bundle_transfers = tree.collect_action_range_filter(
            &bundle_txes,
            TreeSearchBuilder::default().with_action(Actions::is_transfer),
            Actions::try_transfer,
        );

        let balance_deltas = self.get_bundle_accounting(
            bundle_txes,
            bundle_transfers,
            info.tx_index,
            at,
            metadata.clone(),
            mev_type.use_cex_pricing_for_deltas(),
        );

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
            bribe_usd,
            mev_type,
            balance_deltas,
        }
    }

    pub fn get_dex_swaps_rev_usd(
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
        mev_addresses: HashSet<Address>,
        transfers: &[NormalizedTransfer],
        metadata: Arc<Metadata>,
    ) -> Option<Rational> {
        let deltas = self.calculate_transfer_deltas(transfers);

        let addr_usd_deltas =
            self.usd_delta_by_address(tx_index, at, &deltas, metadata.clone(), false)?;

        #[allow(clippy::if_same_then_else)]
        #[allow(clippy::unnecessary_filter_map)]
        //Temporary, waiting on deduplication fix for proper accounting
        let sum = addr_usd_deltas
            .iter()
            .filter_map(
                |(address, delta)| {
                    if mev_addresses.contains(address) {
                        Some(delta)
                    } else {
                        Some(delta)
                    }
                },
            )
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        Some(sum)
    }

    pub fn get_bundle_accounting(
        &self,
        bundle_txes: Vec<FixedBytes<32>>,
        bundle_transfers: Vec<Vec<NormalizedTransfer>>,
        tx_index_for_pricing: u64,
        at: PriceAt,
        metadata: Arc<Metadata>,
        pricing: bool,
    ) -> Vec<TransactionAccounting> {
        bundle_txes
            .into_par_iter()
            .zip(bundle_transfers.into_par_iter())
            .map(|(tx_hash, tx_transfers)| {
                let deltas = self.calculate_transfer_deltas(&tx_transfers);

                let address_deltas: Vec<AddressBalanceDeltas> = deltas
                    .into_iter()
                    .map(|(address, token_deltas)| {
                        let deltas: Vec<TokenBalanceDelta> = token_deltas
                            .into_iter()
                            .map(|(token, amount)| {
                                let usd_value = if pricing {
                                    self.get_token_value_cex(token, amount.clone(), &metadata)
                                        .unwrap_or(Rational::ZERO)
                                } else {
                                    self.get_token_value_dex(
                                        tx_index_for_pricing as usize,
                                        at,
                                        token,
                                        &amount,
                                        &metadata,
                                    )
                                    .unwrap_or(Rational::ZERO)
                                };

                                TokenBalanceDelta {
                                    token:     self
                                        .db
                                        .try_fetch_token_info(token)
                                        .ok()
                                        .unwrap_or_default(),
                                    amount:    amount.clone().to_float(),
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

pub trait TokenAccounting {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas);
}

impl TokenAccounting for Actions {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        match self {
            Actions::Swap(swap) => swap.apply_token_deltas(delta_map),
            Actions::Transfer(transfer) => transfer.apply_token_deltas(delta_map),
            Actions::FlashLoan(flash_loan) => flash_loan.apply_token_deltas(delta_map),
            Actions::Liquidation(liquidation) => liquidation.apply_token_deltas(delta_map),
            Actions::Batch(batch) => batch.apply_token_deltas(delta_map),
            Actions::Burn(burn) => burn.apply_token_deltas(delta_map),
            Actions::Mint(mint) => mint.apply_token_deltas(delta_map),
            Actions::SwapWithFee(swap_with_fee) => swap_with_fee.swap.apply_token_deltas(delta_map),
            Actions::Collect(collect) => collect.apply_token_deltas(delta_map),
            Actions::Unclassified(_) => (), /* Potentially no token deltas to apply, adjust as */
            // necessary
            Actions::SelfDestruct(_self_destruct) => (),
            Actions::EthTransfer(_eth_transfer) => (),
            Actions::NewPool(_new_pool) => (),
            Actions::PoolConfigUpdate(_pool_update) => (),
            Actions::Revert => (), // No token deltas to apply for a revert
        }
    }
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
impl TokenAccounting for NormalizedBurn {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_burned = -amount.clone();
            apply_delta(self.pool, self.token[index].address, amount_burned, delta_map);
            apply_delta(self.recipient, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

impl TokenAccounting for NormalizedMint {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_minted = -amount.clone();
            apply_delta(self.from, self.token[index].address, amount_minted, delta_map);
            apply_delta(self.pool, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

impl TokenAccounting for NormalizedCollect {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_collected = -amount.clone();
            apply_delta(self.pool, self.token[index].address, amount_collected, delta_map);
            apply_delta(self.recipient, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

// fn apply_delta<K: PartialEq + Hash + Eq>(
//     address: K,
impl TokenAccounting for NormalizedFlashLoan {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.child_actions
            .iter()
            .for_each(|action| action.apply_token_deltas(delta_map))
    }
}

impl TokenAccounting for NormalizedLiquidation {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let debt_covered = self.covered_debt.clone();
        // Liquidator sends debt_asset to the pool, effectively swapping the debt asset
        // for the liquidatee's collateral
        apply_delta(self.pool, self.debt_asset.address, debt_covered.clone(), delta_map);
        apply_delta(self.liquidator, self.debt_asset.address, -debt_covered, delta_map);

        // Pool sends collateral to the liquidator
        apply_delta(
            self.pool,
            self.collateral_asset.address,
            -self.liquidated_collateral.clone(),
            delta_map,
        );

        // Liquidator gains collateral asset
        apply_delta(
            self.liquidator,
            self.collateral_asset.address,
            self.liquidated_collateral.clone(),
            delta_map,
        )
    }
}

impl TokenAccounting for NormalizedBatch {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.user_swaps.iter().for_each(|swap| {
            apply_delta(self.solver, swap.token_in.address, swap.amount_in.clone(), delta_map);
            apply_delta(self.solver, swap.token_out.address, -swap.amount_out.clone(), delta_map);

            swap.apply_token_deltas(delta_map);
        });

        if let Some(swaps) = &self.solver_swaps {
            swaps
                .iter()
                .for_each(|swap| swap.apply_token_deltas(delta_map));
        }
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
