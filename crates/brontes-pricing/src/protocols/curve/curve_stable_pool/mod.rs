use std::sync::Arc;

use alloy_primitives::{Address, FixedBytes, Log, B256, U256};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use async_trait::async_trait;
use brontes_types::{
    db::{address_to_protocol_info::ProtocolInfo, traits::LibmdbxReader},
    normalized_actions::Actions,
    traits::TracingProvider,
    ToScaledRational,
};
use malachite::Rational;
use serde::{Deserialize, Serialize};

pub mod batch_request;
pub mod curve_stable_pool_math;
use self::{
    batch_request::get_curve_pool_data_batch_request,
    curve_stable_pool_math::{
        fee_math::calculate_bought_amount_admin_fee,
        stable_swap_invariant::calculate_exchange_rate_stable,
    },
};
use crate::{
    errors::{AmmError, ArithmeticError, EventLogError},
    UpdatableProtocol,
};
sol!(
    #[derive(Debug)]
    interface ICurvePool {
        event TokenExchange(
            address buyer,
            int128 sold_id,
            uint256 tokens_sold,
            int128 bought_id,
            uint256 tokens_bought
        );
        event TokenExchangeUnderlying(
            address buyer,
            int128 sold_id,
            uint256 tokens_sold,
            int128 bought_id,
            uint256 tokens_bought
        );
        event AddLiquidity(
            address provider,
            uint256[] token_amounts,
            uint256[] fees,
            uint256 invariant,
            uint256 token_supply
        );
        event RemoveLiquidity(
            address provider,
            uint256[] token_amounts,
            uint256[] fees, uint256 token_supply
        );
    }
);

// Swap event signature
pub const SWAP_EVENT_SIGNATURE: B256 = FixedBytes([
    139, 62, 150, 242, 184, 137, 250, 119, 28, 83, 201, 129, 180, 13, 175, 0, 95, 99, 246, 55, 241,
    134, 159, 112, 112, 82, 209, 90, 61, 217, 113, 64,
]);

// SwapUnderlying event signature
pub const SWAP_UNDERLYING_EVENT_SIGNATURE: B256 = FixedBytes([
    208, 19, 202, 35, 231, 122, 101, 0, 60, 44, 101, 156, 84, 66, 192, 12, 128, 83, 113, 183, 252,
    30, 189, 76, 32, 108, 65, 209, 83, 107, 217, 11,
]);

// Burn event signature
pub const BURN_EVENT_SIGNATURE: B256 = FixedBytes([
    158, 150, 221, 59, 153, 122, 42, 37, 126, 236, 77, 249, 187, 110, 175, 98, 110, 32, 109, 245,
    245, 67, 189, 150, 54, 130, 209, 67, 48, 11, 227, 16,
]);

// Mint event signature
pub const MINT_EVENT_SIGNATURE: B256 = FixedBytes([
    38, 245, 90, 133, 8, 29, 36, 151, 78, 133, 198, 192, 0, 69, 208, 240, 69, 57, 145, 233, 88,
    115, 245, 43, 255, 13, 33, 175, 64, 121, 167, 104,
]);

#[derive(
    Debug, Clone, Default, Serialize, Deserialize, RlpEncodable, RlpDecodable, Hash, PartialEq, Eq,
)]
pub struct CurvePool {
    pub address:            Address,
    pub tokens:             Vec<Address>,
    pub token_decimals:     Vec<u8>,
    pub fee:                U256,
    pub admin_fee:          U256,
    pub a_value:            U256,
    pub base_pool:          Address,
    pub base_virtual_price: U256,
    pub reserves:           Vec<U256>,
    pub rates:              Vec<U256>,
}

#[async_trait]
impl UpdatableProtocol for CurvePool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_from_action(&mut self, _action: Actions) -> Result<(), AmmError> {
        todo!("syncing from actions is currently not supported for curve stable pools.")
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), AmmError> {
        let event_signature = log.topics()[0];

        (match event_signature {
            BURN_EVENT_SIGNATURE => self.sync_from_burn_log(log),
            MINT_EVENT_SIGNATURE => self.sync_from_mint_log(log),
            SWAP_EVENT_SIGNATURE | SWAP_UNDERLYING_EVENT_SIGNATURE => self.sync_from_swap_log(log),
            _ => Err(EventLogError::InvalidEventSignature.into()),
        })?;

        Ok(())
    }

    fn calculate_price(
        &self,
        base_token: Address,
        quote_token: Option<Address>,
    ) -> Result<Rational, ArithmeticError> {
        let base_token_index = self.tokens.iter().position(|x| x == &base_token);
        let quote_token_index = self.tokens.iter().position(|x| x == &quote_token.unwrap());
        let price = calculate_exchange_rate_stable(
            self.rates[self.tokens.len() - 1],
            self.reserves.clone(),
            self.rates.clone(),
            U256::from(self.tokens.len()),
            quote_token_index.unwrap(),
            base_token_index.unwrap(),
            self.a_value,
        );
        let price_as_rational =
            price.to_scaled_rational(self.token_decimals[base_token_index.unwrap()]);

        Ok(price_as_rational)
    }

    fn tokens(&self) -> Vec<Address> {
        self.tokens.clone()
    }
}

impl CurvePool {
    async fn populate_data<M: TracingProvider>(
        &mut self,
        block: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError> {
        get_curve_pool_data_batch_request(self, block, middleware).await
    }

    // Creates a new instance of the pool from the pair address
    pub async fn new_from_address<M: 'static + TracingProvider, DB: LibmdbxReader>(
        pair_address: Address,
        block_number: u64,
        middleware: Arc<M>,
        db: &DB,
    ) -> Result<Self, AmmError> {
        let pool_details = db.get_protocol_details(pair_address)?;
        let tokens = collect_addresses(&pool_details);
        let mut pool = CurvePool {
            address: pair_address,
            tokens,
            token_decimals: Vec::new(),
            fee: U256::ZERO,
            admin_fee: U256::ZERO,
            a_value: U256::ZERO,
            rates: Vec::new(),
            ..Default::default()
        };

        pool.populate_data(Some(block_number), middleware).await?;

        if !pool.data_is_populated() {
            return Err(AmmError::NoStateError(pair_address))
        }

        Ok(pool)
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.reserves.is_empty() || self.tokens.is_empty())
    }

    fn sync_from_burn_log(&mut self, log: Log) -> Result<(), AmmError> {
        let burn_event = ICurvePool::RemoveLiquidity::decode_log_data(&log, false)?;
        for (i, amount) in burn_event.token_amounts.iter().enumerate() {
            self.reserves[i] -= amount;
        }

        Ok(())
    }

    fn sync_from_mint_log(&mut self, log: Log) -> Result<(), AmmError> {
        let mint_event = ICurvePool::AddLiquidity::decode_log_data(&log, false)?;
        let fee_denominator = U256::from(10).pow(U256::from(10));

        for (i, amount) in mint_event.token_amounts.iter().enumerate() {
            self.reserves[i] += amount;

            let fee_amount = mint_event.fees[i] * (self.admin_fee / fee_denominator);
            self.reserves[i] -= fee_amount;
        }

        Ok(())
    }

    fn sync_from_swap_log(&mut self, log: Log) -> Result<(), AmmError> {
        let event_signature = log.topics()[0];
        let token_exchange_event = ICurvePool::TokenExchange::decode_log_data(&log, false)?;
        let token_exchange_underlying_event =
            ICurvePool::TokenExchangeUnderlying::decode_log_data(&log, false)?;

        let (tokens_sold, tokens_bought, sold_id, bought_id) = match event_signature {
            SWAP_EVENT_SIGNATURE => (
                token_exchange_event.tokens_sold,
                token_exchange_event.tokens_bought,
                token_exchange_event.sold_id,
                token_exchange_event.bought_id,
            ),
            SWAP_UNDERLYING_EVENT_SIGNATURE => (
                token_exchange_underlying_event.tokens_sold,
                token_exchange_underlying_event.tokens_bought,
                token_exchange_underlying_event.sold_id,
                token_exchange_underlying_event.bought_id,
            ),
            _ => {
                return Err(AmmError::EventLogError(EventLogError::InvalidEventSignature));
            }
        };
        self.rates[bought_id as usize - 1] = self.base_virtual_price;
        let bought_amount_admin_fee = calculate_bought_amount_admin_fee(
            self.rates.clone(),
            self.admin_fee,
            self.fee,
            tokens_bought,
            bought_id as usize,
        );

        self.reserves[sold_id as usize - 1] += tokens_sold;
        self.reserves[bought_id as usize - 1] -= tokens_bought + bought_amount_admin_fee;

        Ok(())
    }
}

fn collect_addresses(protocol_info: &ProtocolInfo) -> Vec<Address> {
    let mut addresses = Vec::new();

    addresses.push(protocol_info.token0);
    addresses.push(protocol_info.token1);

    if let Some(token2) = &protocol_info.token2 {
        addresses.push(*token2);
    }
    if let Some(token3) = &protocol_info.token3 {
        addresses.push(*token3);
    }
    if let Some(token4) = &protocol_info.token4 {
        addresses.push(*token4);
    }
    if let Some(curve_lp_token) = &protocol_info.curve_lp_token {
        addresses.push(*curve_lp_token);
    }

    addresses
}
