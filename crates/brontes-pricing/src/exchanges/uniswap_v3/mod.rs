pub mod batch_request;
pub mod factory;

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy_primitives::{Address, BlockNumber, FixedBytes, Log, B256, I256, U256, U64};
use alloy_rlp::{Decodable, Encodable, RlpEncodable};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolEvent};
use async_trait::async_trait;
use brontes_types::{normalized_actions::Actions, traits::TracingProvider, ToScaledRational};
use bytes::BufMut;
use ethers::{
    abi::{ethabi::Bytes, RawLog, Token},
    prelude::{abigen, AbiError, EthEvent},
    types::{Action, Filter},
};
use malachite::{Natural, Rational};
use num_bigfloat::BigFloat;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::make_call_request;
use crate::{
    errors::{AmmError, ArithmeticError, EventLogError, SwapSimulationError},
    exchanges::uniswap_v3::batch_request::get_uniswap_v3_tick_data_batch_request,
    uniswap_v2::IErc20,
    uniswap_v3_math, AutomatedMarketMaker,
};

sol!(
    interface IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
        event PoolCreated(address indexed token0, address indexed token1, uint24 indexed fee, int24 tickSpacing, address pool);
    }
);

sol!(
    interface IUniswapV3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function liquidity() external view returns (uint128);
        function slot0() external view returns (uint160, int24, uint16, uint16, uint16, uint8, bool);
        function fee() external view returns (uint24);
        function tickSpacing() external view returns (int24);
        function ticks(int24 tick) external view returns (uint128, int128, uint256, uint256, int56, uint160, uint32, bool);
        function tickBitmap(int16 wordPosition) external view returns (uint256);
        function swap(address recipient, bool zeroForOne, int256 amountSpecified, uint160 sqrtPriceLimitX96, bytes calldata data) external returns (int256, int256);
        event Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick);
        event Burn(address indexed owner, int24 indexed tickLower, int24 indexed tickUpper, uint128 amount, uint256 amount0, uint256 amount1);
        event Mint(address sender, address indexed owner, int24 indexed tickLower, int24 indexed tickUpper, uint128 amount, uint256 amount0, uint256 amount1);
    }
);

const TASK_LIMIT: usize = 10;

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
    pub tick_bitmap:      HashMap<i16, U256>,
    pub ticks:            HashMap<i32, Info>,

    // non v3 native state
    pub reserve_0: U256,
    pub reserve_1: U256,
}

impl Encodable for UniswapV3Pool {
    fn encode(&self, out: &mut dyn BufMut) {
        self.address.encode(out);
        self.token_a.encode(out);
        self.token_a_decimals.encode(out);
        self.token_b.encode(out);
        self.token_b_decimals.encode(out);
        self.liquidity.encode(out);
        self.sqrt_price.encode(out);
        self.fee.encode(out);
        self.tick.to_be_bytes().encode(out);
        self.tick_spacing.to_be_bytes().encode(out);
        self.tick_bitmap.iter().for_each(|(key, val)| {
            key.to_be_bytes().encode(out);
            val.encode(out);
        });
        self.ticks.iter().for_each(|(key, val)| {
            key.to_be_bytes().encode(out);
            val.encode(out);
        });
        self.reserve_0.encode(out);
        self.reserve_1.encode(out);
    }
}

impl Decodable for UniswapV3Pool {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let address = Address::decode(buf)?;
        let token_a = Address::decode(buf)?;
        let token_a_decimals = u8::decode(buf)?;
        let token_b = Address::decode(buf)?;
        let token_b_decimals = u8::decode(buf)?;
        let liquidity = u128::decode(buf)?;
        let sqrt_price = U256::decode(buf)?;
        let fee = u32::decode(buf)?;
        let tick: [u8; 4] = Decodable::decode(buf)?;
        let tick_spacing: [u8; 4] = Decodable::decode(buf)?;
        let tick_bitmap = Vec::<TickBitMapEncodeHelper>::decode(buf)?
            .into_iter()
            .map(|inner| (inner.key, inner.val))
            .collect::<HashMap<i16, U256>>();
        let ticks = Vec::<TicksEncodeHelper>::decode(buf)?
            .into_iter()
            .map(|inner| (inner.key, inner.val))
            .collect::<HashMap<i32, Info>>();

        let r0 = U256::decode(buf)?;
        let r1 = U256::decode(buf)?;

        Ok(Self {
            address,
            token_a,
            token_a_decimals,
            token_b,
            token_b_decimals,
            liquidity,
            sqrt_price,
            fee,
            tick: i32::from_be_bytes(tick),
            tick_spacing: i32::from_be_bytes(tick_spacing),
            tick_bitmap,
            ticks,
            reserve_0: r0,
            reserve_1: r1,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TickBitMapEncodeHelper {
    key: i16,
    val: U256,
}

impl Encodable for TickBitMapEncodeHelper {
    fn encode(&self, out: &mut dyn BufMut) {
        self.key.to_be_bytes().encode(out);
        self.val.encode(out);
    }
}

impl Decodable for TickBitMapEncodeHelper {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let key: [u8; 2] = Decodable::decode(buf)?;
        let val = U256::decode(buf)?;

        Ok(Self { key: i16::from_be_bytes(key), val })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TicksEncodeHelper {
    key: i32,
    val: Info,
}

impl Encodable for TicksEncodeHelper {
    fn encode(&self, out: &mut dyn BufMut) {
        self.key.to_be_bytes().encode(out);
        self.val.encode(out);
    }
}

impl Decodable for TicksEncodeHelper {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let key: [u8; 4] = Decodable::decode(buf)?;
        let val = Info::decode(buf)?;

        Ok(Self { key: i32::from_be_bytes(key), val })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Info {
    pub liquidity_gross: u128,
    pub liquidity_net:   i128,
    pub initialized:     bool,
}

impl Encodable for Info {
    fn encode(&self, out: &mut dyn BufMut) {
        self.liquidity_gross.encode(out);
        self.liquidity_net.to_be_bytes().encode(out);
        self.initialized.encode(out);
    }
}

impl Decodable for Info {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let liquidity_gross = u128::decode(buf)?;
        let liquidity_net: [u8; 16] = Decodable::decode(buf)?;
        let initialized = bool::decode(buf)?;

        Ok(Self { liquidity_gross, liquidity_net: i128::from_be_bytes(liquidity_net), initialized })
    }
}

impl Info {
    pub fn new(liquidity_gross: u128, liquidity_net: i128, initialized: bool) -> Self {
        Info { liquidity_gross, liquidity_net, initialized }
    }
}

#[async_trait]
impl AutomatedMarketMaker for UniswapV3Pool {
    fn address(&self) -> Address {
        self.address
    }

    fn sync_from_action(&mut self, action: Actions) -> Result<(), EventLogError> {
        todo!()
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
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

    fn calculate_price(&self, base_token: Address) -> Result<f64, ArithmeticError> {
        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(self.sqrt_price)?;
        let shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;

        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(tick) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(tick) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(tick),
        };

        if base_token == self.token_a {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }

    // NOTE: This function will not populate the tick_bitmap and ticks, if you want
    // to populate those, you must call populate_tick_data on an initialized pool
    async fn populate_data<M: TracingProvider>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError> {
        batch_request::get_v3_pool_data_batch_request(self, block_number, middleware.clone())
            .await?;

        let r0 = make_call_request(
            IErc20::balanceOfCall::new((self.address,)),
            middleware.clone(),
            self.token_a,
            block_number,
        )
        .await?;

        let r1 = make_call_request(
            IErc20::balanceOfCall::new((self.address,)),
            middleware,
            self.token_b,
            block_number,
        )
        .await?;

        self.reserve_0 = r0._0;
        self.reserve_1 = r1._0;

        Ok(())
    }

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        tracing::info!(?token_in, ?amount_in, "simulating swap");

        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == self.token_a;

        //Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending
        // on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256::from(1)
        } else {
            MAX_SQRT_RATIO - U256::from(1)
        };

        //Initialize a mutable state state struct to hold the dynamic simulated state
        // of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: self.sqrt_price, //Active price on the pool
            amount_calculated: I256::ZERO,    //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), /* Amount of token_in that
                                               * has not been swapped */
            tick: self.tick,           //Current i24 tick of the pool
            liquidity: self.liquidity, //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            //Initialize a new step struct to hold the dynamic state of the pool at each
            // step
            let mut step = StepComputations {
                sqrt_price_start_x_96: current_state.sqrt_price_x_96, /* Set the sqrt_price_start_x_96 to the current sqrt_price_x_96 */
                ..Default::default()
            };

            //Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &self.tick_bitmap,
                    current_state.tick,
                    self.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not
            // aware of these bounds Note: this could be removed as we are
            // clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            //Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            //Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            //Compute swap step and update the current state
            (current_state.sqrt_price_x_96, step.amount_in, step.amount_out, step.fee_amount) =
                uniswap_v3_math::swap_math::compute_swap_step(
                    current_state.sqrt_price_x_96,
                    swap_target_sqrt_ratio,
                    current_state.liquidity,
                    current_state.amount_specified_remaining,
                    self.fee,
                )?;

            //Decrement the amount remaining to be swapped and amount received from the
            // step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(step.amount_in.overflowing_add(step.fee_amount).0))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            //If the price moved all the way to the next price, recompute the liquidity
            // change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = self.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must
                    // charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(SwapSimulationError::LiquidityUnderflow);
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                //Increment the current tick
                current_state.tick =
                    if zero_for_one { step.tick_next.wrapping_sub(1) } else { step.tick_next }
                //If the current_state sqrt price is not equal to the step sqrt
                // price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the
                // current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        let amount_out = (-current_state.amount_calculated).into_raw();

        tracing::trace!(?amount_out);

        Ok(amount_out)
    }

    fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        tracing::info!(?token_in, ?amount_in, "simulating swap");

        if amount_in.is_zero() {
            return Ok(U256::ZERO);
        }

        let zero_for_one = token_in == self.token_a;

        //Set sqrt_price_limit_x_96 to the max or min sqrt price in the pool depending
        // on zero_for_one
        let sqrt_price_limit_x_96 = if zero_for_one {
            MIN_SQRT_RATIO + U256::from(1)
        } else {
            MAX_SQRT_RATIO - U256::from(1)
        };

        //Initialize a mutable state state struct to hold the dynamic simulated state
        // of the pool
        let mut current_state = CurrentState {
            sqrt_price_x_96: self.sqrt_price, //Active price on the pool
            amount_calculated: I256::ZERO,    //Amount of token_out that has been calculated
            amount_specified_remaining: I256::from_raw(amount_in), /* Amount of token_in that
                                               * has not been swapped */
            tick: self.tick,           //Current i24 tick of the pool
            liquidity: self.liquidity, //Current available liquidity in the tick range
        };

        while current_state.amount_specified_remaining != I256::ZERO
            && current_state.sqrt_price_x_96 != sqrt_price_limit_x_96
        {
            //Initialize a new step struct to hold the dynamic state of the pool at each
            // step
            let mut step = StepComputations {
                sqrt_price_start_x_96: current_state.sqrt_price_x_96, /* Set the sqrt_price_start_x_96 to the current sqrt_price_x_96 */
                ..Default::default()
            };

            //Get the next tick from the current tick
            (step.tick_next, step.initialized) =
                uniswap_v3_math::tick_bitmap::next_initialized_tick_within_one_word(
                    &self.tick_bitmap,
                    current_state.tick,
                    self.tick_spacing,
                    zero_for_one,
                )?;

            // ensure that we do not overshoot the min/max tick, as the tick bitmap is not
            // aware of these bounds Note: this could be removed as we are
            // clamping in the batch contract
            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            //Get the next sqrt price from the input amount
            step.sqrt_price_next_x96 =
                uniswap_v3_math::tick_math::get_sqrt_ratio_at_tick(step.tick_next)?;

            //Target spot price
            let swap_target_sqrt_ratio = if zero_for_one {
                if step.sqrt_price_next_x96 < sqrt_price_limit_x_96 {
                    sqrt_price_limit_x_96
                } else {
                    step.sqrt_price_next_x96
                }
            } else if step.sqrt_price_next_x96 > sqrt_price_limit_x_96 {
                sqrt_price_limit_x_96
            } else {
                step.sqrt_price_next_x96
            };

            //Compute swap step and update the current state
            (current_state.sqrt_price_x_96, step.amount_in, step.amount_out, step.fee_amount) =
                uniswap_v3_math::swap_math::compute_swap_step(
                    current_state.sqrt_price_x_96,
                    swap_target_sqrt_ratio,
                    current_state.liquidity,
                    current_state.amount_specified_remaining,
                    self.fee,
                )?;

            //Decrement the amount remaining to be swapped and amount received from the
            // step
            current_state.amount_specified_remaining = current_state
                .amount_specified_remaining
                .overflowing_sub(I256::from_raw(step.amount_in.overflowing_add(step.fee_amount).0))
                .0;

            current_state.amount_calculated -= I256::from_raw(step.amount_out);

            //If the price moved all the way to the next price, recompute the liquidity
            // change for the next iteration
            if current_state.sqrt_price_x_96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = if let Some(info) = self.ticks.get(&step.tick_next) {
                        info.liquidity_net
                    } else {
                        0
                    };

                    // we are on a tick boundary, and the next tick is initialized, so we must
                    // charge a protocol fee
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    current_state.liquidity = if liquidity_net < 0 {
                        if current_state.liquidity < (-liquidity_net as u128) {
                            return Err(SwapSimulationError::LiquidityUnderflow);
                        } else {
                            current_state.liquidity - (-liquidity_net as u128)
                        }
                    } else {
                        current_state.liquidity + (liquidity_net as u128)
                    };
                }
                //Increment the current tick
                current_state.tick =
                    if zero_for_one { step.tick_next.wrapping_sub(1) } else { step.tick_next }
                //If the current_state sqrt price is not equal to the step sqrt
                // price, then we are not on the same tick.
                // Update the current_state.tick to the tick at the
                // current_state.sqrt_price_x_96
            } else if current_state.sqrt_price_x_96 != step.sqrt_price_start_x_96 {
                current_state.tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(
                    current_state.sqrt_price_x_96,
                )?;
            }
        }

        //Update the pool state
        self.liquidity = current_state.liquidity;
        self.sqrt_price = current_state.sqrt_price_x_96;
        self.tick = current_state.tick;

        let amount_out = (-current_state.amount_calculated).into_raw();

        tracing::trace!(?amount_out);

        Ok(amount_out)
    }

    fn get_token_out(&self, token_in: Address) -> Address {
        if self.token_a == token_in {
            self.token_b
        } else {
            self.token_a
        }
    }
}

impl UniswapV3Pool {
    // #[allow(clippy::too_many_arguments)]
    // pub fn new(
    //     address: Address,
    //     token_a: Address,
    //     token_a_decimals: u8,
    //     token_b: Address,
    //     token_b_decimals: u8,
    //     fee: u32,
    //     liquidity: u128,
    //     sqrt_price: U256,
    //     tick: i32,
    //     tick_spacing: i32,
    //     tick_bitmap: HashMap<i16, U256>,
    //     ticks: HashMap<i32, Info>,
    // ) -> UniswapV3Pool {
    //     UniswapV3Pool {
    //         address,
    //         token_a,
    //         token_a_decimals,
    //         token_b,
    //         token_b_decimals,
    //         fee,
    //         liquidity,
    //         sqrt_price,
    //         tick,
    //         tick_spacing,
    //         tick_bitmap,
    //         ticks,
    //     }
    // }

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
            tick_bitmap: HashMap::new(),
            ticks: HashMap::new(),
            ..Default::default()
        };

        //We need to get tick spacing before populating tick data because tick spacing
        // can not be uninitialized when syncing burn and mint logs
        #[cfg(feature = "uni-v3-ticks")]
        pool.sync_ticks_around_current(block_number, 100, middleware.clone())
            .await;

        pool.populate_data(Some(block_number), middleware).await?;

        if !pool.data_is_populated() {
            return Err(AmmError::PoolDataError);
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
            &self,
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

    // pub async fn new_from_log<M: 'static + TracingProvider>(
    //     log: Log,
    //     middleware: Arc<M>,
    // ) -> Result<Self, AmmError> {
    //     let event_signature = log.topics[0];
    //
    //     if event_signature == POOL_CREATED_EVENT_SIGNATURE {
    //         if let Some(block_number) = log.block_number {
    //             let pool_created=
    // IUniswapV3Factory::PoolCreated::abi_decode_data(&*log.data, false)?;
    //             let pool_created_event =
    // PoolCreatedFilter::decode_log(&RawLog::from(log))?;
    //
    //             UniswapV3Pool::new_from_address(
    //                 pool_created_event.pool,
    //                 block_number.as_u64(),
    //                 middleware,
    //             )
    //             .await
    //         } else {
    //             Err(EventLogError::LogBlockNumberNotFound)?
    //         }
    //     } else {
    //         Err(EventLogError::InvalidEventSignature)?
    //     }
    // }
    //
    // pub fn new_empty_pool_from_log(log: Log) -> Result<Self, EventLogError> {
    //     let event_signature = log.topics[0];
    //
    //     if event_signature == POOL_CREATED_EVENT_SIGNATURE {
    //         let pool_created_event =
    // PoolCreatedFilter::decode_log(&RawLog::from(log))?;
    //
    //         Ok(UniswapV3Pool {
    //             address:          pool_created_event.pool,
    //             token_a:          pool_created_event.token_0,
    //             token_b:          pool_created_event.token_1,
    //             token_a_decimals: 0,
    //             token_b_decimals: 0,
    //             fee:              pool_created_event.fee,
    //             liquidity:        0,
    //             sqrt_price:       U256::ZERO,
    //             tick_spacing:     0,
    //             tick:             0,
    //             tick_bitmap:      HashMap::new(),
    //             ticks:            HashMap::new(),
    //         })
    //     } else {
    //         Err(EventLogError::InvalidEventSignature)
    //     }
    // }
    //
    // pub async fn populate_tick_data<M: 'static + TracingProvider>(
    //     &mut self,
    //     mut from_block: u64,
    //     middleware: Arc<M>,
    // ) -> Result<u64, AmmError> {
    //     let current_block = middleware
    //         .get_block_number()
    //         .await
    //         .map_err(AMMError::TracingProviderError)?
    //         .as_u64();
    //     let mut ordered_logs: BTreeMap<U64, Vec<Log>> = BTreeMap::new();
    //
    //     let pool_address: Address = self.address;
    //
    //     let mut handles = vec![];
    //     let mut tasks = 0;
    //
    //     while from_block < current_block {
    //         let middleware = middleware.clone();
    //
    //         let mut target_block = from_block + POPULATE_TICK_DATA_STEP - 1;
    //         if target_block > current_block {
    //             target_block = current_block;
    //         }
    //
    //         handles.push(tokio::spawn(async move {
    //             let logs = middleware
    //                 .get_logs(
    //                     &Filter::new()
    //                         .topic0(vec![BURN_EVENT_SIGNATURE,
    // MINT_EVENT_SIGNATURE])                         .address(pool_address)
    //                         .from_block(BlockNumber::Number(U64([from_block])))
    //                         .to_block(BlockNumber::Number(U64([target_block]))),
    //                 )
    //                 .await
    //                 .map_err(AMMError::TracingProviderError)?;
    //
    //             Ok::<Vec<Log>, AmmError>(logs)
    //         }));
    //
    //         from_block += POPULATE_TICK_DATA_STEP;
    //         tasks += 1;
    //         //Here we are limiting the number of green threads that can be spun
    // up to not         // have the node time out
    //         if tasks == TASK_LIMIT {
    //             self.process_logs_from_handles(handles, &mut ordered_logs)
    //                 .await?;
    //             handles = vec![];
    //             tasks = 0;
    //         }
    //     }
    //
    //     self.process_logs_from_handles(handles, &mut ordered_logs)
    //         .await?;
    //
    //     for (_, log_group) in ordered_logs {
    //         for log in log_group {
    //             self.sync_from_log(log)?;
    //         }
    //     }
    //
    //     Ok(current_block)
    // }
    //
    // async fn process_logs_from_handles<M: TracingProvider>(
    //     &self,
    //     handles: Vec<JoinHandle<Result<Vec<Log>, AmmError>>>,
    //     ordered_logs: &mut BTreeMap<U64, Vec<Log>>,
    // ) -> Result<(), AmmError> {
    //     // group the logs from each thread by block number and then sync the logs
    // in     // chronological order
    //     for handle in handles {
    //         let logs = handle.await??;
    //
    //         for log in logs {
    //             if let Some(log_block_number) = log.block_number {
    //                 if let Some(log_group) =
    // ordered_logs.get_mut(&log_block_number) {
    // log_group.push(log);                 } else {
    //                     ordered_logs.insert(log_block_number, vec![log]);
    //                 }
    //             } else {
    //                 return Err(EventLogError::LogBlockNumberNotFound)?;
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    pub fn fee(&self) -> u32 {
        self.fee
    }

    pub fn data_is_populated(&self) -> bool {
        !(self.token_a.is_zero() || self.token_b.is_zero())
    }

    // pub async fn get_tick_word<M: TracingProvider>(
    //     &self,
    //     tick: i32,
    //     middleware: Arc<M>,
    // ) -> Result<U256, AmmError> {
    //     // let v3_pool = IUniswapV3Pool::new(self.address, middleware);
    //     let (word_position, _) = uniswap_v3_math::tick_bitmap::position(tick);
    //
    //     let call =
    // IUniswapV3Pool::tickBitmapCall::new(word_position).abi_encode();     call
    //     Ok(v3_pool.tick_bitmap(word_position).call().await?)
    // }
    //
    // pub async fn get_next_word<M: TracingProvider>(
    //     &self,
    //     word_position: i16,
    //     middleware: Arc<M>,
    // ) -> Result<U256, AmmError> {
    //     let v3_pool = IUniswapV3Pool::new(self.address, middleware);
    //     Ok(v3_pool.tick_bitmap(word_position).call().await?)
    // }
    //
    pub async fn get_tick_spacing<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
    ) -> Result<i32, AmmError> {
        let call = IUniswapV3Pool::tickSpacingCall::new(());
        let res = make_call_request(call, middleware, self.address, None).await?;
        Ok(res._0)
    }

    pub async fn get_tick<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
        block: u64,
    ) -> Result<i32, AmmError> {
        Ok(self.get_slot_0(middleware, block).await?._1)
    }

    // pub async fn get_tick_info<M: TracingProvider>(
    //     &self,
    //     tick: i32,
    //     middleware: Arc<M>,
    // ) -> Result<(u128, i128, U256, U256, i64, U256, u32, bool), AmmError> {
    //     let v3_pool = IUniswapV3Pool::new(self.address, middleware.clone());
    //
    //     let tick_info = v3_pool.ticks(tick).call().await?;
    //
    //     Ok((
    //         tick_info.0,
    //         tick_info.1,
    //         tick_info.2,
    //         tick_info.3,
    //         tick_info.4,
    //         tick_info.5,
    //         tick_info.6,
    //         tick_info.7,
    //     ))
    // }
    //
    // pub async fn get_liquidity_net<M: TracingProvider>(
    //     &self,
    //     tick: i32,
    //     middleware: Arc<M>,
    // ) -> Result<i128, AmmError> {
    //     let tick_info = self.get_tick_info(tick, middleware).await?;
    //     Ok(tick_info.1)
    // }
    //
    // pub async fn get_initialized<M: TracingProvider>(
    //     &self,
    //     tick: i32,
    //     middleware: Arc<M>,
    // ) -> Result<bool, AmmError> {
    //     let tick_info = self.get_tick_info(tick, middleware).await?;
    //     Ok(tick_info.7)
    // }
    //
    pub async fn get_slot_0<M: TracingProvider>(
        &self,
        middleware: Arc<M>,
        block: u64,
    ) -> Result<IUniswapV3Pool::slot0Return, AmmError> {
        Ok(make_call_request(
            IUniswapV3Pool::slot0Call::new(()),
            middleware,
            self.address,
            Some(block),
        )
        .await?)
    }

    //
    // pub async fn get_liquidity<M: TracingProvider>(
    //     &self,
    //     middleware: Arc<M>,
    // ) -> Result<u128, AmmError> {
    //     let v3_pool = IUniswapV3Pool::new(self.address, middleware);
    //     Ok(v3_pool.liquidity().call().await?)
    // }
    //
    // pub async fn get_sqrt_price<M: TracingProvider>(
    //     &self,
    //     middleware: Arc<M>,
    // ) -> Result<U256, AmmError> {
    //     Ok(self.get_slot_0(middleware).await?.0)
    // }
    //
    pub fn sync_from_burn_log(&mut self, log: Log) -> Result<(), AbiError> {
        let burn_event = IUniswapV3Pool::Burn::decode_log_object(&log, false).unwrap();

        self.modify_position(
            burn_event.tickLower,
            burn_event.tickUpper,
            -(burn_event.amount as i128),
        );

        Ok(())
    }

    pub fn sync_from_mint_log(&mut self, log: Log) -> Result<(), AbiError> {
        let mint_event = IUniswapV3Pool::Mint::decode_log_object(&log, false).unwrap();

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

    pub fn sync_from_swap_log(&mut self, log: Log) -> Result<(), AbiError> {
        let swap_event = IUniswapV3Pool::Swap::decode_log_object(&log, false).unwrap();

        self.sqrt_price = swap_event.sqrtPriceX96;
        self.liquidity = swap_event.liquidity;
        self.tick = swap_event.tick;

        Ok(())
    }

    pub fn get_tvl(&self) -> Rational {
        self.reserve_0.to_scaled_rational(0) + self.reserve_1.to_scaled_rational(0)
    }

    //
    // pub async fn get_token_decimals<M: TracingProvider>(
    //     &mut self,
    //     middleware: Arc<M>,
    // ) -> Result<(u8, u8), AmmError> {
    //     let token_a_decimals = IErc20::new(self.token_a, middleware.clone())
    //         .decimals()
    //         .call()
    //         .await?;
    //
    //     let token_b_decimals = IErc20::new(self.token_b, middleware)
    //         .decimals()
    //         .call()
    //         .await?;
    //
    //     Ok((token_a_decimals, token_b_decimals))
    // }
    //
    // pub async fn get_fee<M: TracingProvider>(&mut self, middleware: Arc<M>) ->
    // Result<u32, AmmError> {     let fee =
    // IUniswapV3Pool::new(self.address, middleware)         .fee()
    //         .call()
    //         .await?;
    //
    //     Ok(fee)
    // }
    //
    // pub async fn get_token_0<M: TracingProvider>(
    //     &self,
    //     middleware: Arc<M>,
    // ) -> Result<Address, AmmError> {
    //     let v3_pool = IUniswapV3Pool::new(self.address, middleware);
    //
    //     let token_0 = match v3_pool.token_0().call().await {
    //         Ok(result) => result,
    //         Err(contract_error) => return
    // Err(AMMError::ContractError(contract_error)),     };
    //
    //     Ok(token_0)
    // }
    //
    // pub async fn get_token_1<M: TracingProvider>(
    //     &self,
    //     middleware: Arc<M>,
    // ) -> Result<Address, AmmError> {
    //     let v3_pool = IUniswapV3Pool::new(self.address, middleware);
    //
    //     let token_1 = match v3_pool.token_1().call().await {
    //         Ok(result) => result,
    //         Err(contract_error) => return
    // Err(AMMError::ContractError(contract_error)),     };
    //
    //     Ok(token_1)
    // }

    /* Legend:
       sqrt(price) = sqrt(y/x)
       L = sqrt(x*y)
       ==> x = L^2/price
       ==> y = L^2*price
    */
    pub fn calculate_virtual_reserves(&self) -> Result<(u128, u128), ArithmeticError> {
        let tick = uniswap_v3_math::tick_math::get_tick_at_sqrt_ratio(self.sqrt_price)?;
        let price = 1.0001_f64.powi(tick);

        let sqrt_price = BigFloat::from_f64(price.sqrt());
        let liquidity = BigFloat::from_u128(self.liquidity);

        let (reserve_0, reserve_1) = if !sqrt_price.is_zero() {
            let reserve_x = liquidity.div(&sqrt_price);
            let reserve_y = liquidity.mul(&sqrt_price);

            (reserve_x, reserve_y)
        } else {
            (BigFloat::from(0), BigFloat::from(0))
        };

        Ok((
            reserve_0
                .to_u128()
                .ok_or(ArithmeticError::U128ConversionError)?,
            reserve_1
                .to_u128()
                .ok_or(ArithmeticError::U128ConversionError)?,
        ))
    }

    pub fn calculate_compressed(&self, tick: i32) -> i32 {
        if tick < 0 && tick % self.tick_spacing != 0 {
            (tick / self.tick_spacing) - 1
        } else {
            tick / self.tick_spacing
        }
    }

    pub fn calculate_word_pos_bit_pos(&self, compressed: i32) -> (i16, u8) {
        uniswap_v3_math::tick_bitmap::position(compressed)
    }

    pub fn swap_calldata(
        &self,
        recipient: Address,
        zero_for_one: bool,
        amount_specified: I256,
        sqrt_price_limit_x_96: U256,
        calldata: Vec<u8>,
    ) -> Result<Bytes, ethers::abi::Error> {
        Ok(IUniswapV3Pool::swapCall::new((
            recipient,
            zero_for_one,
            amount_specified,
            sqrt_price_limit_x_96,
            calldata,
        ))
        .abi_encode())
    }
}

pub struct CurrentState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x_96: U256,
    tick: i32,
    liquidity: u128,
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

const MIN_TICK: i32 = -887272;
const MAX_TICK: i32 = 887272;

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

// #[cfg(test)]
// mod test {
//     #[allow(unused)]
//     use std::error::Error;
//     #[allow(unused)]
//     use std::{str::FromStr, sync::Arc};
//
//     #[allow(unused)]
//     use ethers::{
//         prelude::abigen,
//         providers::{Http, Provider},
//         types::{Address, U256},
//     };
//
//     use super::IUniswapV3Pool;
//     #[allow(unused)]
//     #[allow(unused)]
//     use super::UniswapV3Pool;
//     use crate::amm::AutomatedMarketMaker;
//     abigen!(
//         IQuoter,
//     r#"[
//         function quoteExactInputSingle(address tokenIn, address
// tokenOut,uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external
// returns (uint256 amountOut)     ]"#;);
//
//     async fn initialize_usdc_weth_pool<M: 'static + TracingProvider>(
//         middleware: Arc<M>,
//     ) -> eyre::Result<(UniswapV3Pool, u64)> {
//         let mut pool = UniswapV3Pool {
//             address:
// Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
// ..Default::default()         };
//
//         let creation_block = 12369620;
//         pool.tick_spacing = pool.get_tick_spacing(middleware.clone()).await?;
//         let synced_block = pool
//             .populate_tick_data(creation_block, middleware.clone())
//             .await?;
//         pool.populate_data(Some(synced_block), middleware).await?;
//
//         Ok((pool, synced_block))
//     }
//
//     async fn initialize_weth_link_pool<M: 'static + TracingProvider>(
//         middleware: Arc<M>,
//     ) -> eyre::Result<(UniswapV3Pool, u64)> {
//         let mut pool = UniswapV3Pool {
//             address:
// Address::from_str("0xa6Cc3C2531FdaA6Ae1A3CA84c2855806728693e8")?,
// ..Default::default()         };
//
//         let creation_block = 12375680;
//         pool.tick_spacing = pool.get_tick_spacing(middleware.clone()).await?;
//         let synced_block = pool
//             .populate_tick_data(creation_block, middleware.clone())
//             .await?;
//         pool.populate_data(Some(synced_block), middleware).await?;
//
//         Ok((pool, synced_block))
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_usdc_weth() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_usdc_weth_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//         let amount_in = U256::from_dec_str("100000000")?; // 100 USDC
//
//         let amount_out = pool.simulate_swap(pool.token_a, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000")?; // 10_000 USDC
//
//         let amount_out_1 = pool.simulate_swap(pool.token_a, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("10000000000000")?; //
// 10_000_000 USDC
//
//         let amount_out_2 = pool.simulate_swap(pool.token_a, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000")?; //
// 100_000_000 USDC
//
//         let amount_out_3 = pool.simulate_swap(pool.token_a, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_weth_usdc() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_usdc_weth_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 ETH
//
//         let amount_out = pool.simulate_swap(pool.token_b, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000000000000")?; // 10
// ETH
//
//         let amount_out_1 = pool.simulate_swap(pool.token_b, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("100000000000000000000")?; //
// 100 ETH
//
//         let amount_out_2 = pool.simulate_swap(pool.token_b, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000000000")?; //
// 100_000 ETH
//
//         let amount_out_3 = pool.simulate_swap(pool.token_b, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_link_weth() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_weth_link_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 LINK
//
//         let amount_out = pool.simulate_swap(pool.token_a, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("100000000000000000000")?; //
// 100 LINK
//
//         let amount_out_1 = pool.simulate_swap(pool.token_a, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("10000000000000000000000")?; //
// 10_000 LINK
//
//         let amount_out_2 = pool.simulate_swap(pool.token_a, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("10000000000000000000000")?; //
// 1_000_000 LINK
//
//         let amount_out_3 = pool.simulate_swap(pool.token_a, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_weth_link() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_weth_link_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 ETH
//
//         let amount_out = pool.simulate_swap(pool.token_b, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000000000000")?; // 10
// ETH
//
//         let amount_out_1 = pool.simulate_swap(pool.token_b, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("100000000000000000000")?; //
// 100 ETH
//
//         let amount_out_2 = pool.simulate_swap(pool.token_b, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000000000")?; //
// 100_000 ETH
//
//         let amount_out_3 = pool.simulate_swap(pool.token_b, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_mut_usdc_weth() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_usdc_weth_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//         let amount_in = U256::from_dec_str("100000000")?; // 100 USDC
//
//         let amount_out = pool.simulate_swap(pool.token_a, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000")?; // 10_000 USDC
//
//         let amount_out_1 = pool.simulate_swap(pool.token_a, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("10000000000000")?; //
// 10_000_000 USDC
//
//         let amount_out_2 = pool.simulate_swap(pool.token_a, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000")?; //
// 100_000_000 USDC
//
//         let amount_out_3 = pool.simulate_swap(pool.token_a, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_mut_weth_usdc() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_usdc_weth_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 ETH
//
//         let amount_out = pool.simulate_swap(pool.token_b, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000000000000")?; // 10
// ETH
//
//         let amount_out_1 = pool.simulate_swap(pool.token_b, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("100000000000000000000")?; //
// 100 ETH
//
//         let amount_out_2 = pool.simulate_swap(pool.token_b, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000000000")?; //
// 100_000 ETH
//
//         let amount_out_3 = pool.simulate_swap(pool.token_b, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_mut_link_weth() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_weth_link_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 LINK
//
//         let amount_out = pool.simulate_swap(pool.token_a, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("100000000000000000000")?; //
// 100 LINK
//
//         let amount_out_1 = pool.simulate_swap(pool.token_a, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("10000000000000000000000")?; //
// 10_000 LINK
//
//         let amount_out_2 = pool.simulate_swap(pool.token_a, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("10000000000000000000000")?; //
// 1_000_000 LINK
//
//         let amount_out_3 = pool.simulate_swap(pool.token_a, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_a, pool.token_b, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_simulate_swap_mut_weth_link() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, synced_block) =
// initialize_weth_link_pool(middleware.clone()).await?;         let quoter =
// IQuoter::new(
// Address::from_str("0xb27308f9f90d607463bb33ea1bebb41c27ce5ab6")?,
// middleware.clone(),         );
//
//         let amount_in = U256::from_dec_str("1000000000000000000")?; // 1 ETH
//
//         let amount_out = pool.simulate_swap(pool.token_b, amount_in)?;
//         let expected_amount_out = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//         assert_eq!(amount_out, expected_amount_out);
//         let amount_in_1 = U256::from_dec_str("10000000000000000000")?; // 10
// ETH
//
//         let amount_out_1 = pool.simulate_swap(pool.token_b, amount_in_1)?;
//
//         let expected_amount_out_1 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_1, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_1, expected_amount_out_1);
//
//         let amount_in_2 = U256::from_dec_str("100000000000000000000")?; //
// 100 ETH
//
//         let amount_out_2 = pool.simulate_swap(pool.token_b, amount_in_2)?;
//
//         let expected_amount_out_2 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_2, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_2, expected_amount_out_2);
//
//         let amount_in_3 = U256::from_dec_str("100000000000000000000")?; //
// 100_000 ETH
//
//         let amount_out_3 = pool.simulate_swap(pool.token_b, amount_in_3)?;
//
//         let expected_amount_out_3 = quoter
//             .quote_exact_input_single(pool.token_b, pool.token_a, pool.fee,
// amount_in_3, U256::ZERO)             .block(synced_block)
//             .call()
//             .await?;
//
//         assert_eq!(amount_out_3, expected_amount_out_3);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_get_new_from_address() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let pool = UniswapV3Pool::new_from_address(
//             Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
//             12369620,
//             middleware.clone(),
//         )
//         .await?;
//
//         assert_eq!(pool.address,
// Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640")?);
//         assert_eq!(pool.token_a,
// Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?);
//         assert_eq!(pool.token_a_decimals, 6);
//         assert_eq!(pool.token_b,
// Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")?);
//         assert_eq!(pool.token_b_decimals, 18);
//         assert_eq!(pool.fee, 500);
//         assert!(pool.tick != 0);
//         assert_eq!(pool.tick_spacing, 10);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_get_pool_data() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let (pool, _synced_block) =
// initialize_usdc_weth_pool(middleware.clone()).await?;         assert_eq!
// (pool.address,
// Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640")?);
//         assert_eq!(pool.token_a,
// Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?);
//         assert_eq!(pool.token_a_decimals, 6);
//         assert_eq!(pool.token_b,
// Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")?);
//         assert_eq!(pool.token_b_decimals, 18);
//         assert_eq!(pool.fee, 500);
//         assert!(pool.tick != 0);
//         assert_eq!(pool.tick_spacing, 10);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_sync_pool() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let mut pool = UniswapV3Pool {
//             address:
// Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
// ..Default::default()         };
//
//         pool.sync(middleware).await?;
//
//         //TODO: need to assert values
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_calculate_virtual_reserves() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let mut pool = UniswapV3Pool {
//             address:
// Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
// ..Default::default()         };
//
//         pool.populate_data(None, middleware.clone()).await?;
//
//         let pool_at_block = IUniswapV3Pool::new(
//             Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
//             middleware.clone(),
//         );
//
//         let sqrt_price =
// pool_at_block.slot_0().block(16515398).call().await?.0;         let liquidity
// = pool_at_block.liquidity().block(16515398).call().await?;
//
//         pool.sqrt_price = sqrt_price;
//         pool.liquidity = liquidity;
//
//         let (r_0, r_1) = pool.calculate_virtual_reserves()?;
//
//         assert_eq!(1067543429906214, r_0);
//         assert_eq!(649198362624067343572319, r_1);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_calculate_price() -> eyre::Result<()> {
//         let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
//         let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);
//
//         let mut pool = UniswapV3Pool {
//             address:
// Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
// ..Default::default()         };
//
//         pool.populate_data(None, middleware.clone()).await?;
//
//         let block_pool = IUniswapV3Pool::new(
//             Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")?,
//             middleware.clone(),
//         );
//
//         let sqrt_price = block_pool.slot_0().block(16515398).call().await?.0;
//         pool.sqrt_price = sqrt_price;
//
//         let float_price_a = pool.calculate_price(pool.token_a)?;
//         let float_price_b = pool.calculate_price(pool.token_b)?;
//
//         assert_eq!(float_price_a, 0.0006081236083117488);
//         assert_eq!(float_price_b, 1644.4025299004006);
//
//         Ok(())
//     }
// }
