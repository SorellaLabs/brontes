pub mod batch_request;
pub mod factory;

use std::sync::Arc;

use alloy_primitives::{Address, FixedBytes, Log, B256};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use async_trait::async_trait;
use brontes_types::{normalized_actions::Action, traits::TracingProvider, ToScaledRational};
use malachite::{
    num::{arithmetic::traits::Pow, basic::traits::Zero},
    Natural, Rational,
};
use serde::{Deserialize, Serialize};

use self::batch_request::get_v2_pool_data;
use crate::{
    errors::{AmmError, ArithmeticError, EventLogError},
    UpdatableProtocol,
};

sol!(
    #[derive(Debug)]
    interface IUniswapV2Pair {
        function getReserves() external view returns (
            uint112 reserve0,
            uint112 reserve1,
            uint32 blockTimestampLast
        );
        function token0() external view returns (address);
        function token1() external view returns (address);
        function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
        event Sync(uint112 reserve0, uint112 reserve1);
    }
);

sol!(
    interface IErc20 {
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
    }
);
pub const U128_0X10000000000000000: u128 = 18446744073709551616;
pub const SYNC_EVENT_SIGNATURE: B256 = FixedBytes([
    28, 65, 30, 154, 150, 224, 113, 36, 28, 47, 33, 247, 114, 107, 23, 174, 137, 227, 202, 180,
    199, 139, 229, 14, 6, 43, 3, 169, 255, 251, 186, 209,
]);

#[derive(
    Debug, Clone, Default, Serialize, Deserialize, RlpEncodable, RlpDecodable, Hash, PartialEq, Eq,
)]
pub struct UniswapV2Pool {
    pub address:          Address,
    pub token_a:          Address,
    pub token_a_decimals: u8,
    pub token_b:          Address,
    pub token_b_decimals: u8,
    pub reserve_0:        u128,
    pub reserve_1:        u128,
    pub fee:              u32,
}

#[async_trait]
impl UpdatableProtocol for UniswapV2Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_from_action(&mut self, _action: Action) -> Result<(), AmmError> {
        todo!("syncing from actions is currently not supported for v2")
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), AmmError> {
        let event_signature = log.topics()[0];

        if event_signature == SYNC_EVENT_SIGNATURE {
            let sync_event = IUniswapV2Pair::Sync::decode_log_data(&log, false)?;

            self.reserve_0 = sync_event.reserve0.to();
            self.reserve_1 = sync_event.reserve1.to();

            Ok(())
        } else {
            Err(AmmError::EventLogError(EventLogError::InvalidEventSignature))
        }
    }

    //Calculates base/quote, meaning the price of base token per quote (ie.
    // exchange rate is X base per 1 quote)
    fn calculate_price(&self, base_token: Address) -> Result<Rational, ArithmeticError> {
        self.calculate_price_64_x_64(base_token)
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }
}

impl UniswapV2Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: Address,
        token_a: Address,
        token_a_decimals: u8,
        token_b: Address,
        token_b_decimals: u8,
        reserve_0: u128,
        reserve_1: u128,
        fee: u32,
    ) -> UniswapV2Pool {
        UniswapV2Pool {
            address,
            token_a,
            token_a_decimals,
            token_b,
            token_b_decimals,
            reserve_0,
            reserve_1,
            fee,
        }
    }

    pub async fn populate_data<M: TracingProvider>(
        &mut self,
        block: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError> {
        get_v2_pool_data(self, block, middleware).await
    }

    pub async fn new_load_on_block<M: TracingProvider>(
        pair_addr: Address,
        middleware: Arc<M>,
        block: u64,
    ) -> Result<Self, AmmError> {
        let mut pool = UniswapV2Pool {
            address:          pair_addr,
            token_a:          Address::ZERO,
            token_a_decimals: 0,
            token_b:          Address::ZERO,
            token_b_decimals: 0,
            reserve_0:        0,
            reserve_1:        0,
            fee:              0,
        };

        pool.populate_data(Some(block), middleware).await?;

        if !pool.data_is_populated() {
            return Err(AmmError::NoStateError(pair_addr));
        }

        Ok(pool)
    }

    //Creates a new instance of the pool from the pair address, and syncs the pool
    // data
    pub async fn new_from_address<M: TracingProvider>(
        pair_address: Address,
        fee: u32,
        middleware: Arc<M>,
    ) -> Result<Self, AmmError> {
        let mut pool = UniswapV2Pool {
            address: pair_address,
            token_a: Address::ZERO,
            token_a_decimals: 0,
            token_b: Address::ZERO,
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee,
        };

        pool.populate_data(None, middleware.clone()).await?;

        if !pool.data_is_populated() {
            return Err(AmmError::PoolDataError);
        }

        Ok(pool)
    }

    pub fn fee(&self) -> u32 {
        self.fee
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero()
            || self.token_b.is_zero()
            || self.reserve_0 == 0
            || self.reserve_1 == 0)
    }

    pub fn calculate_price_64_x_64(
        &self,
        base_token: Address,
    ) -> Result<Rational, ArithmeticError> {
        let (r_0, r_1) = (
            Rational::from_naturals(
                Natural::from(self.reserve_0),
                Natural::from(10u64).pow(self.token_a_decimals as u64),
            ),
            Rational::from_naturals(
                Natural::from(self.reserve_1),
                Natural::from(10u64).pow(self.token_b_decimals as u64),
            ),
        );

        if r_0 == Rational::ZERO || r_1 == Rational::ZERO {
            return Err(ArithmeticError::UniV2DivZero);
        }

        if base_token == self.token_a {
            Ok(r_1 / r_0)
        } else {
            Ok(r_0 / r_1)
        }
    }

    pub fn get_tvl(&self, base: Address) -> (Rational, Rational) {
        if self.token_a == base {
            (
                self.reserve_0.to_scaled_rational(self.token_a_decimals),
                self.reserve_1.to_scaled_rational(self.token_b_decimals),
            )
        } else {
            (
                self.reserve_1.to_scaled_rational(self.token_b_decimals),
                self.reserve_0.to_scaled_rational(self.token_a_decimals),
            )
        }
    }
}
