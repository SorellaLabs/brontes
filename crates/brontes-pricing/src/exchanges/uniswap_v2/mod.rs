pub mod batch_request;
pub mod factory;

use std::sync::Arc;

use amms::{
    amm::AutomatedMarketMaker,
    errors::{AMMError, ArithmeticError, EventLogError, SwapSimulationError},
};
use async_trait::async_trait;
use ethers::{
    abi::{ethabi::Bytes, RawLog, Token},
    prelude::{abigen, EthEvent},
    providers::Middleware,
    types::{Log, H160, H256, U256},
};
use num_bigfloat::BigFloat;
use serde::{Deserialize, Serialize};

use self::factory::PAIR_CREATED_EVENT_SIGNATURE;

abigen!(
    IUniswapV2Pair,
    r#"[
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        function token0() external view returns (address)
        function token1() external view returns (address)
        function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data);
        event Sync(uint112 reserve0, uint112 reserve1)
    ]"#;

    IErc20,
    r#"[
        function balanceOf(address account) external view returns (uint256)
        function decimals() external view returns (uint8)
    ]"#;
);

pub const U128_0X10000000000000000: u128 = 18446744073709551616;
pub const SYNC_EVENT_SIGNATURE: H256 = H256([
    28, 65, 30, 154, 150, 224, 113, 36, 28, 47, 33, 247, 114, 107, 23, 174, 137, 227, 202, 180,
    199, 139, 229, 14, 6, 43, 3, 169, 255, 251, 186, 209,
]);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address:          H160,
    pub token_a:          H160,
    pub token_a_decimals: u8,
    pub token_b:          H160,
    pub token_b_decimals: u8,
    pub reserve_0:        u128,
    pub reserve_1:        u128,
    pub fee:              u32,
}

#[async_trait]
impl AutomatedMarketMaker for UniswapV2Pool {
    fn address(&self) -> H160 {
        self.address
    }

    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), AMMError<M>> {
        (self.reserve_0, self.reserve_1) = self.get_reserves(middleware).await?;

        Ok(())
    }

    async fn populate_data<M: Middleware>(
        &mut self,
        _block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AMMError<M>> {
        batch_request::get_v2_pool_data_batch_request(self, middleware.clone()).await?;

        Ok(())
    }

    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![SYNC_EVENT_SIGNATURE]
    }

    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        let event_signature = log.topics[0];

        if event_signature == SYNC_EVENT_SIGNATURE {
            let sync_event = SyncFilter::decode_log(&RawLog::from(log))?;

            self.reserve_0 = sync_event.reserve_0;
            self.reserve_1 = sync_event.reserve_1;

            Ok(())
        } else {
            Err(EventLogError::InvalidEventSignature)
        }
    }

    //Calculates base/quote, meaning the price of base token per quote (ie.
    // exchange rate is X base per 1 quote)
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        Ok(q64_to_f64(self.calculate_price_64_x_64(base_token)?))
    }

    fn tokens(&self) -> Vec<H160> {
        vec![self.token_a, self.token_b]
    }

    fn simulate_swap(&self, token_in: H160, amount_in: U256) -> Result<U256, SwapSimulationError> {
        tracing::info!(?token_in, ?amount_in, "simulating swap");

        if self.token_a == token_in {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            ))
        } else {
            Ok(self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            ))
        }
    }

    fn simulate_swap_mut(
        &mut self,
        token_in: H160,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError> {
        tracing::info!(?token_in, ?amount_in, "simulating swap");

        if self.token_a == token_in {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_0),
                U256::from(self.reserve_1),
            );

            tracing::trace!(?amount_out);
            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves before");

            self.reserve_0 += amount_in.as_u128();
            self.reserve_1 -= amount_out.as_u128();

            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves after");

            Ok(amount_out)
        } else {
            let amount_out = self.get_amount_out(
                amount_in,
                U256::from(self.reserve_1),
                U256::from(self.reserve_0),
            );

            tracing::trace!(?amount_out);
            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves before");

            self.reserve_0 -= amount_out.as_u128();
            self.reserve_1 += amount_in.as_u128();

            tracing::trace!(?self.reserve_0, ?self.reserve_1, "pool reserves after");

            Ok(amount_out)
        }
    }

    fn get_token_out(&self, token_in: H160) -> H160 {
        if self.token_a == token_in {
            self.token_b
        } else {
            self.token_a
        }
    }
}

impl UniswapV2Pool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: H160,
        token_a: H160,
        token_a_decimals: u8,
        token_b: H160,
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

    //Creates a new instance of the pool from the pair address, and syncs the pool
    // data
    pub async fn new_from_address<M: Middleware>(
        pair_address: H160,
        fee: u32,
        middleware: Arc<M>,
    ) -> Result<Self, AMMError<M>> {
        let mut pool = UniswapV2Pool {
            address: pair_address,
            token_a: H160::zero(),
            token_a_decimals: 0,
            token_b: H160::zero(),
            token_b_decimals: 0,
            reserve_0: 0,
            reserve_1: 0,
            fee,
        };

        pool.populate_data(None, middleware.clone()).await?;

        if !pool.data_is_populated() {
            return Err(AMMError::PoolDataError)
        }

        Ok(pool)
    }

    pub async fn new_from_log<M: Middleware>(
        log: Log,
        fee: u32,
        middleware: Arc<M>,
    ) -> Result<Self, AMMError<M>> {
        let event_signature = log.topics[0];

        if event_signature == PAIR_CREATED_EVENT_SIGNATURE {
            let pair_created_event = factory::PairCreatedFilter::decode_log(&RawLog::from(log))?;
            UniswapV2Pool::new_from_address(pair_created_event.pair, fee, middleware).await
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }
    }

    pub fn new_empty_pool_from_log(log: Log) -> Result<Self, EventLogError> {
        let event_signature = log.topics[0];

        if event_signature == PAIR_CREATED_EVENT_SIGNATURE {
            let pair_created_event = factory::PairCreatedFilter::decode_log(&RawLog::from(log))?;

            Ok(UniswapV2Pool {
                address:          pair_created_event.pair,
                token_a:          pair_created_event.token_0,
                token_b:          pair_created_event.token_1,
                token_a_decimals: 0,
                token_b_decimals: 0,
                reserve_0:        0,
                reserve_1:        0,
                fee:              0,
            })
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }
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

    pub async fn get_reserves<M: Middleware>(
        &self,
        middleware: Arc<M>,
    ) -> Result<(u128, u128), AMMError<M>> {
        tracing::trace!("getting reserves of {}", self.address);

        //Initialize a new instance of the Pool
        let v2_pair = IUniswapV2Pair::new(self.address, middleware);
        // Make a call to get the reserves
        let (reserve_0, reserve_1, _) = match v2_pair.get_reserves().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        tracing::trace!(reserve_0, reserve_1);

        Ok((reserve_0, reserve_1))
    }

    pub async fn get_token_decimals<M: Middleware>(
        &mut self,
        middleware: Arc<M>,
    ) -> Result<(u8, u8), AMMError<M>> {
        let token_a_decimals = IErc20::new(self.token_a, middleware.clone())
            .decimals()
            .call()
            .await?;

        let token_b_decimals = IErc20::new(self.token_b, middleware)
            .decimals()
            .call()
            .await?;

        tracing::trace!(token_a_decimals, token_b_decimals);

        Ok((token_a_decimals, token_b_decimals))
    }

    pub async fn get_token_0<M: Middleware>(
        &self,
        pair_address: H160,
        middleware: Arc<M>,
    ) -> Result<H160, AMMError<M>> {
        let v2_pair = IUniswapV2Pair::new(pair_address, middleware);

        let token0 = match v2_pair.token_0().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        Ok(token0)
    }

    pub async fn get_token_1<M: Middleware>(
        &self,
        pair_address: H160,
        middleware: Arc<M>,
    ) -> Result<H160, AMMError<M>> {
        let v2_pair = IUniswapV2Pair::new(pair_address, middleware);

        let token1 = match v2_pair.token_1().call().await {
            Ok(result) => result,
            Err(contract_error) => return Err(AMMError::ContractError(contract_error)),
        };

        Ok(token1)
    }

    pub fn calculate_price_64_x_64(&self, base_token: H160) -> Result<u128, ArithmeticError> {
        let decimal_shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;

        let (r_0, r_1) = if decimal_shift < 0 {
            (
                U256::from(self.reserve_0)
                    * U256::from(10u128.pow(decimal_shift.unsigned_abs() as u32)),
                U256::from(self.reserve_1),
            )
        } else {
            (
                U256::from(self.reserve_0),
                U256::from(self.reserve_1) * U256::from(10u128.pow(decimal_shift as u32)),
            )
        };

        if base_token == self.token_a {
            if r_0.is_zero() {
                Ok(U128_0X10000000000000000)
            } else {
                div_uu(r_1, r_0)
            }
        } else if r_1.is_zero() {
            Ok(U128_0X10000000000000000)
        } else {
            div_uu(r_0, r_1)
        }
    }

    pub fn get_amount_out(&self, amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        tracing::trace!(?amount_in, ?reserve_in, ?reserve_out);

        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::zero()
        }
        let fee = (10000 - (self.fee / 10)) / 10; //Fee of 300 => (10,000 - 30) / 10  = 997
        let amount_in_with_fee = amount_in * U256::from(fee);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        tracing::trace!(?fee, ?amount_in_with_fee, ?numerator, ?denominator);

        numerator / denominator
    }

    pub fn swap_calldata(
        &self,
        amount_0_out: U256,
        amount_1_out: U256,
        to: H160,
        calldata: Vec<u8>,
    ) -> Result<Bytes, ethers::abi::Error> {
        let input_tokens = vec![
            Token::Uint(amount_0_out),
            Token::Uint(amount_1_out),
            Token::Address(to),
            Token::Bytes(calldata),
        ];

        IUNISWAPV2PAIR_ABI
            .function("swap")?
            .encode_input(&input_tokens)
    }
}

pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 =
    U256([18446744073709551615, 18446744073709551615, 18446744073709551615, 0]);

pub const U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: U256 =
    U256([18446744073709551615, 18446744073709551615, 0, 0]);

pub const U256_0X100000000: U256 = U256([4294967296, 0, 0, 0]);
pub const U256_0X10000: U256 = U256([65536, 0, 0, 0]);
pub const U256_0X100: U256 = U256([256, 0, 0, 0]);
pub const U256_255: U256 = U256([255, 0, 0, 0]);
pub const U256_192: U256 = U256([192, 0, 0, 0]);
pub const U256_191: U256 = U256([191, 0, 0, 0]);
pub const U256_128: U256 = U256([128, 0, 0, 0]);
pub const U256_64: U256 = U256([64, 0, 0, 0]);
pub const U256_32: U256 = U256([32, 0, 0, 0]);
pub const U256_16: U256 = U256([16, 0, 0, 0]);
pub const U256_8: U256 = U256([8, 0, 0, 0]);
pub const U256_4: U256 = U256([4, 0, 0, 0]);
pub const U256_2: U256 = U256([2, 0, 0, 0]);

pub fn div_uu(x: U256, y: U256) -> Result<u128, ArithmeticError> {
    if !y.is_zero() {
        let mut answer;

        if x <= U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            answer = (x << U256_64) / y;
        } else {
            let mut msb = U256_192;
            let mut xc = x >> U256_192;

            if xc >= U256_0X100000000 {
                xc >>= U256_32;
                msb += U256_32;
            }

            if xc >= U256_0X10000 {
                xc >>= U256_16;
                msb += U256_16;
            }

            if xc >= U256_0X100 {
                xc >>= U256_8;
                msb += U256_8;
            }

            if xc >= U256_16 {
                xc >>= U256_4;
                msb += U256_4;
            }

            if xc >= U256_4 {
                xc >>= U256_2;
                msb += U256_2;
            }

            if xc >= U256_2 {
                msb += U256::one();
            }

            answer =
                (x << (U256_255 - msb)) / (((y - U256::one()) >> (msb - U256_191)) + U256::one());
        }

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Err(ArithmeticError::ShadowOverflow(answer))
        }

        let hi = answer * (y >> U256_128);
        let mut lo = answer * (y & U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);

        let mut xh = x >> U256_192;
        let mut xl = x << U256_64;

        if xl < lo {
            xh -= U256::one();
        }

        xl = xl.overflowing_sub(lo).0;
        lo = hi << U256_128;

        if xl < lo {
            xh -= U256::one();
        }

        xl = xl.overflowing_sub(lo).0;

        if xh != hi >> U256_128 {
            return Err(ArithmeticError::RoundingError)
        }

        answer += xl / y;

        if answer > U256_0XFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF {
            return Err(ArithmeticError::ShadowOverflow(answer))
        }

        Ok(answer.as_u128())
    } else {
        Err(ArithmeticError::YIsZero)
    }
}

//Converts a Q64 fixed point to a Q16 fixed point -> f64
pub fn q64_to_f64(x: u128) -> f64 {
    BigFloat::from(x)
        .div(&BigFloat::from(U128_0X10000000000000000))
        .to_f64()
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use ethers::{
        providers::{Http, Provider},
        types::{H160, U256},
    };

    use super::UniswapV2Pool;
    use crate::amm::AutomatedMarketMaker;

    #[test]
    fn test_swap_calldata() -> eyre::Result<()> {
        let uniswap_v2_pool = UniswapV2Pool::default();

        let _calldata = uniswap_v2_pool.swap_calldata(
            U256::from(123456789),
            U256::zero(),
            H160::from_str("0x41c36f504BE664982e7519480409Caf36EE4f008")?,
            vec![],
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_get_new_from_address() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

        let pool = UniswapV2Pool::new_from_address(
            H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?,
            300,
            middleware.clone(),
        )
        .await?;

        assert_eq!(pool.address, H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?);
        assert_eq!(pool.token_a, H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?);
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(pool.token_b, H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")?);
        assert_eq!(pool.token_b_decimals, 18);
        assert_eq!(pool.fee, 300);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_pool_data() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

        let mut pool = UniswapV2Pool {
            address: H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?,
            ..Default::default()
        };

        pool.populate_data(None, middleware.clone()).await?;

        assert_eq!(pool.address, H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?);
        assert_eq!(pool.token_a, H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")?);
        assert_eq!(pool.token_a_decimals, 6);
        assert_eq!(pool.token_b, H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")?);
        assert_eq!(pool.token_b_decimals, 18);

        Ok(())
    }

    #[test]
    fn test_calculate_price_edge_case() -> eyre::Result<()> {
        let token_a = H160::from_str("0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270")?;
        let token_b = H160::from_str("0x8f18dc399594b451eda8c5da02d0563c0b2d0f16")?;
        let x = UniswapV2Pool {
            address: H160::from_str("0x652a7b75c229850714d4a11e856052aac3e9b065")?,
            token_a,
            token_a_decimals: 18,
            token_b,
            token_b_decimals: 9,
            reserve_0: 23595096345912178729927,
            reserve_1: 154664232014390554564,
            fee: 300,
        };

        assert!(x.calculate_price(token_a)? != 0.0);
        assert!(x.calculate_price(token_b)? != 0.0);

        Ok(())
    }
    #[tokio::test]
    async fn test_calculate_price() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

        let mut pool = UniswapV2Pool {
            address: H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?,
            ..Default::default()
        };

        pool.populate_data(None, middleware.clone()).await?;

        pool.reserve_0 = 47092140895915;
        pool.reserve_1 = 28396598565590008529300;

        let price_a_64_x = pool.calculate_price(pool.token_a)?;

        let price_b_64_x = pool.calculate_price(pool.token_b)?;

        assert_eq!(1658.3725965327264, price_b_64_x); //No precision loss: 30591574867092394336528 / 2**64
        assert_eq!(0.0006030007985483893, price_a_64_x); //Precision loss: 11123401407064628 / 2**64

        Ok(())
    }
    #[tokio::test]
    async fn test_calculate_price_64_x_64() -> eyre::Result<()> {
        let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
        let middleware = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

        let mut pool = UniswapV2Pool {
            address: H160::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")?,
            ..Default::default()
        };

        pool.populate_data(None, middleware.clone()).await?;

        pool.reserve_0 = 47092140895915;
        pool.reserve_1 = 28396598565590008529300;

        let price_a_64_x = pool.calculate_price_64_x_64(pool.token_a)?;

        let price_b_64_x = pool.calculate_price_64_x_64(pool.token_b)?;

        assert_eq!(30591574867092394336528, price_b_64_x);
        assert_eq!(11123401407064628, price_a_64_x);

        Ok(())
    }
}
