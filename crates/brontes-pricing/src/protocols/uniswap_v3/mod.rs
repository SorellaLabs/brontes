pub mod batch_request;
pub mod uniswap_v3_math;
use std::{cmp::Ordering, sync::Arc};

use alloy_primitives::{Address, FixedBytes, Log, B256, U256};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolEvent};
use async_trait::async_trait;
use brontes_types::{
    normalized_actions::Action, traits::TracingProvider, FastHashMap, ToScaledRational,
};
use malachite::Rational;
use serde::{Deserialize, Serialize};

use self::batch_request::get_v3_pool_data_batch_request;
use super::make_call_request;
#[cfg(feature = "uni-v3-ticks")]
use crate::uniswap_v3::batch_request::get_uniswap_v3_tick_data_batch_request;
#[cfg(feature = "uni-v3-ticks")]
use crate::uniswap_v3::uniswap_v3_math::tick_math::{MAX_TICK, MIN_TICK};
use crate::{
    errors::{AmmError, ArithmeticError, EventLogError},
    UpdatableProtocol,
};

sol!(
    interface IUniswapV3Factory {
        function getPool(
            address tokenA,
            address tokenB,
            uint24 fee
        ) external view returns (address pool);
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24 indexed fee,
            int24 tickSpacing,
            address pool
        );
    }
);

sol!(
    interface IUniswapV3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function liquidity() external view returns (uint128);
        function slot0() external view returns
            (uint160, int24, uint16, uint16, uint16, uint8, bool);
        function fee() external view returns (uint24);
        function tickSpacing() external view returns (int24);
        function ticks(int24 tick) external view returns (
            uint128,
            int128,
            uint256,
            uint256,
            int56,
            uint160,
            uint32,
            bool
        );
        function tickBitmap(int16 wordPosition) external view returns (uint256);
        function swap(
            address recipient,
            bool zeroForOne,
            int256 amountSpecified,
            uint160 sqrtPriceLimitX96,
            bytes calldata data
        ) external returns (int256, int256);

        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );
        event Burn(
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );
        event Mint(
            address sender,
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );
    }
);

pub const MIN_SQRT_RATIO: U256 = U256::from_limbs([4295128739, 0, 0, 0]);
pub const MAX_SQRT_RATIO: U256 =
    U256::from_limbs([6743328256752651558, 17280870778742802505, 4294805859, 0]);
pub const POPULATE_TICK_DATA_STEP: u64 = 100000;

pub const U256_TWO: U256 = U256::from_limbs([2, 0, 0, 0]);
pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q224: U256 = U256::from_limbs([0, 0, 0, 4294967296]);

pub const SWAP_EVENT_SIGNATURE: B256 = FixedBytes([
    196, 32, 121, 249, 74, 99, 80, 215, 230, 35, 95, 41, 23, 73, 36, 249, 40, 204, 42, 200, 24,
    235, 100, 254, 216, 0, 78, 17, 95, 188, 202, 103,
]);

// Burn event signature
pub const BURN_EVENT_SIGNATURE: B256 = FixedBytes([
    12, 57, 108, 217, 137, 163, 159, 68, 89, 181, 250, 26, 237, 106, 154, 141, 205, 188, 69, 144,
    138, 207, 214, 126, 2, 140, 213, 104, 218, 152, 152, 44,
]);

// Mint event signature
pub const MINT_EVENT_SIGNATURE: B256 = FixedBytes([
    122, 83, 8, 11, 164, 20, 21, 139, 231, 236, 105, 185, 135, 181, 251, 125, 7, 222, 225, 1, 254,
    133, 72, 143, 8, 83, 174, 22, 35, 157, 11, 222,
]);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniswapV3Pool {
    pub address:          Address,
    pub token_a:          Address,
    pub token_a_decimals: u8,
    pub token_b:          Address,
    pub token_b_decimals: u8,
    pub liquidity:        u128,
    pub sqrt_price:       U256,
    pub fee:              u32,
    pub tick:             i32,
    pub tick_spacing:     i32,
    pub tick_bitmap:      FastHashMap<i16, U256>,
    pub ticks:            FastHashMap<i32, Info>,

    // non v3 native state
    pub reserve_0: U256,
    pub reserve_1: U256,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Info {
    pub liquidity_gross: u128,
    pub liquidity_net:   i128,
    pub initialized:     bool,
}

impl Info {
    pub fn new(liquidity_gross: u128, liquidity_net: i128, initialized: bool) -> Self {
        Info { liquidity_gross, liquidity_net, initialized }
    }
}

#[async_trait]
impl UpdatableProtocol for UniswapV3Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_from_action(&mut self, _action: Action) -> Result<(), AmmError> {
        todo!("syncing from actions is currently not supported for v3")
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), AmmError> {
        let event_signature = log.topics()[0];

        if event_signature == BURN_EVENT_SIGNATURE {
            self.sync_from_burn_log(log)?;
        } else if event_signature == MINT_EVENT_SIGNATURE {
            self.sync_from_mint_log(log)?;
        } else if event_signature == SWAP_EVENT_SIGNATURE {
            self.sync_from_swap_log(log)?;
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }

        Ok(())
    }

    fn tokens(&self) -> Vec<Address> {
        vec![self.token_a, self.token_b]
    }

    fn calculate_price(&self, base_token: Address) -> Result<Rational, ArithmeticError> {
        if self.liquidity <= 10_000 {
            return Err(ArithmeticError::UniswapV3MathError(
                uniswap_v3_math::error::UniswapV3MathError::LiquidityTooLow(self.liquidity),
            ))
        }

        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(self.sqrt_price)?;
        let shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;
        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(tick) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(tick) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(tick),
        };

        if base_token == self.token_a {
            Ok(Rational::try_from(price).unwrap())
        } else {
            Ok(Rational::try_from(1.0 / price).unwrap())
        }
    }
}

impl UniswapV3Pool {
    async fn populate_data<M: TracingProvider>(
        &mut self,
        block: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError> {
        get_v3_pool_data_batch_request(self, block, middleware).await
    }

    // Creates a new instance of the pool from the pair address
    pub async fn new_from_address<M: 'static + TracingProvider>(
        pair_address: Address,
        block_number: u64,
        middleware: Arc<M>,
    ) -> Result<Self, AmmError> {
        let mut pool = UniswapV3Pool {
            address: pair_address,
            token_a: Address::ZERO,
            token_a_decimals: 0,
            token_b: Address::ZERO,
            token_b_decimals: 0,
            liquidity: 0,
            sqrt_price: U256::ZERO,
            tick: 0,
            tick_spacing: 0,
            fee: 0,
            tick_bitmap: FastHashMap::default(),
            ticks: FastHashMap::default(),
            ..Default::default()
        };

        //We need to get tick spacing before populating tick data because tick spacing
        // can not be uninitialized when syncing burn and mint logs
        #[cfg(feature = "uni-v3-ticks")]
        pool.sync_ticks_around_current(block_number, 100, middleware.clone())
            .await;

        pool.populate_data(Some(block_number), middleware).await?;

        if !pool.data_is_populated() {
            return Err(AmmError::NoStateError(pair_address))
        }

        Ok(pool)
    }

    #[cfg(feature = "uni-v3-ticks")]
    pub async fn sync_ticks_around_current<M: 'static + TracingProvider>(
        &mut self,
        block: u64,
        tick_amount: i32,
        provider: Arc<M>,
    ) {
        if tick_amount.is_negative() {
            return
        }

        if self.tick == 0 {
            self.tick = self.get_tick(provider.clone(), block).await.unwrap();
        }

        let cur_tick = self.tick;

        let (start_tick, am) = if self.tick - tick_amount < MIN_TICK {
            (MIN_TICK, tick_amount * 2)
        } else if self.tick + tick_amount > MAX_TICK {
            // cur_tick - max_tick_delta we can support
            let left_shift = MAX_TICK - self.tick;
            (self.tick - left_shift, left_shift * 2)
        } else {
            (cur_tick - tick_amount, tick_amount * 2)
        };
        let ticks = get_uniswap_v3_tick_data_batch_request(
            self,
            start_tick,
            true,
            am as u16,
            Some(block),
            provider,
        )
        .await
        .unwrap()
        .0;

        for tick in ticks {
            self.update_tick(tick.tick, tick.liquidityNet, tick.initialized);
        }
    }

    pub fn fee(&self) -> u32 {
        self.fee
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero() || self.token_b.is_zero())
            || !(self.sqrt_price >= MIN_SQRT_RATIO && self.sqrt_price < MAX_SQRT_RATIO)
    }

    pub async fn get_tick_spacing<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
    ) -> Result<i32, AmmError> {
        let call = IUniswapV3Pool::tickSpacingCall::new(());
        let res = make_call_request(call, &middleware, self.address, None).await?;
        Ok(res._0.as_i32())
    }

    pub async fn get_tick<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
        block: u64,
    ) -> Result<i32, AmmError> {
        Ok(self.get_slot_0(middleware, block).await?._1.as_i32())
    }

    pub async fn get_slot_0<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
        block: u64,
    ) -> Result<IUniswapV3Pool::slot0Return, AmmError> {
        Ok(make_call_request(
            IUniswapV3Pool::slot0Call::new(()),
            &middleware,
            self.address,
            Some(block),
        )
        .await?)
    }

    pub fn sync_from_burn_log(&mut self, log: Log) -> Result<(), AmmError> {
        let burn_event = IUniswapV3Pool::Burn::decode_log_data(&log, false)?;
        self.reserve_0 -= burn_event.amount0;
        self.reserve_1 -= burn_event.amount1;

        #[cfg(feature = "uni-v3-ticks")]
        self.modify_position(
            burn_event.tickLower,
            burn_event.tickUpper,
            -(burn_event.amount as i128),
        );

        Ok(())
    }

    pub fn sync_from_mint_log(&mut self, log: Log) -> Result<(), AmmError> {
        let mint_event = IUniswapV3Pool::Mint::decode_log_data(&log, false)?;

        self.reserve_0 += mint_event.amount0;
        self.reserve_1 += mint_event.amount1;

        #[cfg(feature = "uni-v3-ticks")]
        self.modify_position(mint_event.tickLower, mint_event.tickUpper, mint_event.amount as i128);

        Ok(())
    }

    pub fn modify_position(&mut self, tick_lower: i32, tick_upper: i32, liquidity_delta: i128) {
        //We are only using this function when a mint or burn event is emitted,
        //therefore we do not need to checkTicks as that has happened before the event
        // is emitted
        self.update_position(tick_lower, tick_upper, liquidity_delta);

        if liquidity_delta != 0 {
            //if the tick is between the tick lower and tick upper, update the liquidity
            // between the ticks
            if self.tick > tick_lower && self.tick < tick_upper {
                self.liquidity = if liquidity_delta < 0 {
                    self.liquidity - ((-liquidity_delta) as u128)
                } else {
                    self.liquidity + (liquidity_delta as u128)
                }
            }
        }
    }

    pub fn update_position(&mut self, tick_lower: i32, tick_upper: i32, liquidity_delta: i128) {
        let mut flipped_lower = false;
        let mut flipped_upper = false;

        if liquidity_delta != 0 {
            flipped_lower = self.update_tick(tick_lower, liquidity_delta, false);
            flipped_upper = self.update_tick(tick_upper, liquidity_delta, true);
            if flipped_lower {
                self.flip_tick(tick_lower, self.tick_spacing);
            }
            if flipped_upper {
                self.flip_tick(tick_upper, self.tick_spacing);
            }
        }

        if liquidity_delta < 0 {
            if flipped_lower {
                self.ticks.remove(&tick_lower);
            }

            if flipped_upper {
                self.ticks.remove(&tick_upper);
            }
        }
    }

    pub fn update_tick(&mut self, tick: i32, liquidity_delta: i128, upper: bool) -> bool {
        let info = match self.ticks.get_mut(&tick) {
            Some(info) => info,
            None => {
                self.ticks.insert(tick, Info::default());
                self.ticks
                    .get_mut(&tick)
                    .expect("Tick does not exist in ticks")
            }
        };

        let liquidity_gross_before = info.liquidity_gross;

        let liquidity_gross_after = if liquidity_delta < 0 {
            liquidity_gross_before - ((-liquidity_delta) as u128)
        } else {
            liquidity_gross_before + (liquidity_delta as u128)
        };

        //we do not need to check if liqudity_gross_after > maxLiquidity because we are
        // only calling update tick on a burn or mint log. this should already
        // be validated when a log is
        let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

        if liquidity_gross_before == 0 {
            info.initialized = true;
        }

        info.liquidity_gross = liquidity_gross_after;

        info.liquidity_net = if upper {
            info.liquidity_net - liquidity_delta
        } else {
            info.liquidity_net + liquidity_delta
        };

        flipped
    }

    pub fn flip_tick(&mut self, tick: i32, tick_spacing: i32) {
        let (word_pos, bit_pos) = uniswap_v3_math::tick_bitmap::position(tick / tick_spacing);
        let mask = U256::from(1) << bit_pos;

        if let Some(word) = self.tick_bitmap.get_mut(&word_pos) {
            *word ^= mask;
        } else {
            self.tick_bitmap.insert(word_pos, mask);
        }
    }

    pub fn sync_from_swap_log(&mut self, log: Log) -> Result<(), AmmError> {
        let swap_event = IUniswapV3Pool::Swap::decode_log_data(&log, false)?;

        if swap_event.amount0.is_negative() {
            self.reserve_0 -= swap_event.amount0.unsigned_abs();
            self.reserve_1 += swap_event.amount1.unsigned_abs();
        } else {
            self.reserve_0 += swap_event.amount0.unsigned_abs();
            self.reserve_1 -= swap_event.amount1.unsigned_abs();
        }

        self.sqrt_price = U256::from(swap_event.sqrtPriceX96);
        self.liquidity = swap_event.liquidity;
        self.tick = swap_event.tick.as_i32();

        Ok(())
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

#[derive(Default)]
pub struct StepComputations {
    pub sqrt_price_start_x_96: U256,
    pub tick_next:             i32,
    pub initialized:           bool,
    pub sqrt_price_next_x96:   U256,
    pub amount_in:             U256,
    pub amount_out:            U256,
    pub fee_amount:            U256,
}

pub struct Tick {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_x_128: U256,
    pub fee_growth_outside_1_x_128: U256,
    pub tick_cumulative_outside: U256,
    pub seconds_per_liquidity_outside_x_128: U256,
    pub seconds_outside: u32,
    pub initialized: bool,
}
