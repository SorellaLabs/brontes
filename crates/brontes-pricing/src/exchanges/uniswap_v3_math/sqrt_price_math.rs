use alloy_primitives::{I256, U256};

use super::{
    error::UniswapV3MathError,
    full_math::{mul_div, mul_div_rounding_up},
    unsafe_math::div_rounding_up,
};

pub const MAX_U160: U256 =
    U256::from_limbs([18446744073709551615, 18446744073709551615, 4294967295, 0]);
pub const Q96: U256 = U256::from_limbs([0, 4294967296, 0, 0]);
pub const FIXED_POINT_96_RESOLUTION: U256 = U256::from_limbs([96, 0, 0, 0]);

// returns (sqrtQX96)
pub fn get_next_sqrt_price_from_input(
    sqrt_price: U256,
    liquidity: u128,
    amount_in: U256,
    zero_for_one: bool,
) -> Result<U256, UniswapV3MathError> {
    if sqrt_price.is_zero() {
        return Err(UniswapV3MathError::SqrtPriceIsZero)
    } else if liquidity == 0 {
        return Err(UniswapV3MathError::LiquidityIsZero)
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price, liquidity, amount_in, true)
    } else {
        get_next_sqrt_price_from_amount_1_rounding_down(sqrt_price, liquidity, amount_in, true)
    }
}

// returns (sqrtQX96)
pub fn get_next_sqrt_price_from_output(
    sqrt_price: U256,
    liquidity: u128,
    amount_out: U256,
    zero_for_one: bool,
) -> Result<U256, UniswapV3MathError> {
    if sqrt_price.is_zero() {
        return Err(UniswapV3MathError::SqrtPriceIsZero)
    } else if liquidity == 0 {
        return Err(UniswapV3MathError::LiquidityIsZero)
    }

    if zero_for_one {
        get_next_sqrt_price_from_amount_1_rounding_down(sqrt_price, liquidity, amount_out, false)
    } else {
        get_next_sqrt_price_from_amount_0_rounding_up(sqrt_price, liquidity, amount_out, false)
    }
}

// returns (uint160 sqrtQX96)
pub fn get_next_sqrt_price_from_amount_0_rounding_up(
    sqrt_price_x_96: U256,
    liquidity: u128,
    amount: U256,
    add: bool,
) -> Result<U256, UniswapV3MathError> {
    if amount.is_zero() {
        return Ok(sqrt_price_x_96)
    }

    let numerator_1 = U256::from(liquidity) << 96;
    let amount = amount;
    let sqrt_price_x_96 = sqrt_price_x_96;

    if add {
        let product = amount.wrapping_mul(sqrt_price_x_96);

        if product.wrapping_div(amount) == sqrt_price_x_96 {
            let denominator = numerator_1.wrapping_add(product);

            if denominator >= numerator_1 {
                return mul_div_rounding_up(numerator_1, sqrt_price_x_96, denominator)
            }
        }

        Ok(div_rounding_up(
            numerator_1,
            (numerator_1.wrapping_div(sqrt_price_x_96)).wrapping_add(amount),
        ))
    } else {
        let product = amount.wrapping_mul(sqrt_price_x_96);
        if product.wrapping_div(amount) == sqrt_price_x_96 && numerator_1 > product {
            let denominator = numerator_1.wrapping_sub(product);

            mul_div_rounding_up(numerator_1, sqrt_price_x_96, denominator)
        } else {
            Err(UniswapV3MathError::ProductDivAmount)
        }
    }
}

// returns (uint160 sqrtQX96)
pub fn get_next_sqrt_price_from_amount_1_rounding_down(
    sqrt_price_x_96: U256,
    liquidity: u128,
    amount: U256,
    add: bool,
) -> Result<U256, UniswapV3MathError> {
    if add {
        let quotient = if amount <= MAX_U160 {
            (amount << FIXED_POINT_96_RESOLUTION) / liquidity
        } else {
            mul_div(amount, Q96, U256::from(liquidity))?
        };

        let next_sqrt_price = sqrt_price_x_96 + quotient;

        if next_sqrt_price > MAX_U160 {
            Err(UniswapV3MathError::SafeCastToU160Overflow)
        } else {
            Ok(next_sqrt_price)
        }
    } else {
        let quotient = if amount <= MAX_U160 {
            div_rounding_up(amount << FIXED_POINT_96_RESOLUTION, U256::from(liquidity))
        } else {
            mul_div_rounding_up(amount, Q96, U256::from(liquidity))?
        };

        //require(sqrtPX96 > quotient);
        if sqrt_price_x_96 <= quotient {
            return Err(UniswapV3MathError::SqrtPriceIsLteQuotient)
        }

        Ok(sqrt_price_x_96.overflowing_sub(quotient).0)
    }
}

// returns (uint256 amount0)
pub fn _get_amount_0_delta(
    mut sqrt_ratio_a_x_96: U256,
    mut sqrt_ratio_b_x_96: U256,
    liquidity: u128,
    round_up: bool,
) -> Result<U256, UniswapV3MathError> {
    if sqrt_ratio_a_x_96 > sqrt_ratio_b_x_96 {
        (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) = (sqrt_ratio_b_x_96, sqrt_ratio_a_x_96)
    };

    let numerator_1 = U256::from(liquidity) << 96;
    let numerator_2 = sqrt_ratio_b_x_96 - sqrt_ratio_a_x_96;

    if sqrt_ratio_a_x_96.is_zero() {
        return Err(UniswapV3MathError::SqrtPriceIsZero)
    }

    if round_up {
        let numerator_partial = mul_div_rounding_up(numerator_1, numerator_2, sqrt_ratio_b_x_96)?;
        Ok(div_rounding_up(numerator_partial, sqrt_ratio_a_x_96))
    } else {
        Ok(mul_div(numerator_1, numerator_2, sqrt_ratio_b_x_96)? / sqrt_ratio_a_x_96)
    }
}

// returns (uint256 amount1)
pub fn _get_amount_1_delta(
    mut sqrt_ratio_a_x_96: U256,
    mut sqrt_ratio_b_x_96: U256,
    liquidity: u128,
    round_up: bool,
) -> Result<U256, UniswapV3MathError> {
    if sqrt_ratio_a_x_96 > sqrt_ratio_b_x_96 {
        (sqrt_ratio_a_x_96, sqrt_ratio_b_x_96) = (sqrt_ratio_b_x_96, sqrt_ratio_a_x_96)
    };

    if round_up {
        mul_div_rounding_up(
            U256::from(liquidity),
            sqrt_ratio_b_x_96 - sqrt_ratio_a_x_96,
            U256::from("0x1000000000000000000000000"),
        )
    } else {
        mul_div(
            U256::from(liquidity),
            sqrt_ratio_b_x_96 - sqrt_ratio_a_x_96,
            U256::from("0x1000000000000000000000000"),
        )
    }
}

pub fn get_amount_0_delta(
    sqrt_ratio_a_x_96: U256,
    sqrt_ratio_b_x_96: U256,
    liquidity: i128,
) -> Result<I256, UniswapV3MathError> {
    if liquidity < 0 {
        Ok(-I256::from_raw(_get_amount_0_delta(
            sqrt_ratio_a_x_96,
            sqrt_ratio_b_x_96,
            -liquidity as u128,
            false,
        )?))
    } else {
        Ok(I256::from_raw(_get_amount_0_delta(
            sqrt_ratio_a_x_96,
            sqrt_ratio_b_x_96,
            liquidity as u128,
            true,
        )?))
    }
}

pub fn get_amount_1_delta(
    sqrt_ratio_a_x_96: U256,
    sqrt_ratio_b_x_96: U256,
    liquidity: i128,
) -> Result<I256, UniswapV3MathError> {
    if liquidity < 0 {
        Ok(-I256::from_raw(_get_amount_1_delta(
            sqrt_ratio_a_x_96,
            sqrt_ratio_b_x_96,
            -liquidity as u128,
            false,
        )?))
    } else {
        Ok(I256::from_raw(_get_amount_1_delta(
            sqrt_ratio_a_x_96,
            sqrt_ratio_b_x_96,
            liquidity as u128,
            true,
        )?))
    }
}

#[cfg(test)]
mod test {
    use std::ops::{Add, Sub};

    use ethers::types::U256;

    use super::{_get_amount_0_delta, get_next_sqrt_price_from_input};
    use crate::sqrt_price_math::{_get_amount_1_delta, get_next_sqrt_price_from_output, MAX_U160};

    #[test]
    fn test_get_next_sqrt_price_from_input() {
        //Fails if price is zero
        let result = get_next_sqrt_price_from_input(
            U256::zero(),
            0,
            U256::from(100000000000000000_u128),
            false,
        );
        assert_eq!(result.unwrap_err().to_string(), "Sqrt price is 0");

        //Fails if liquidity is zero
        let result = get_next_sqrt_price_from_input(
            U256::one(),
            0,
            U256::from(100000000000000000_u128),
            true,
        );
        assert_eq!(result.unwrap_err().to_string(), "Liquidity is 0");

        //fails if input amount overflows the price
        let result = get_next_sqrt_price_from_input(MAX_U160, 1024, U256::from(1024), false);
        assert_eq!(result.unwrap_err().to_string(), "Overflow when casting to U160");

        //any input amount cannot underflow the price
        let result = get_next_sqrt_price_from_input(
            U256::one(),
            1,
            U256::from_dec_str(
                "57896044618658097711785492504343953926634992332820282019728792003956564819968",
            )
            .unwrap(),
            true,
        );

        assert_eq!(result.unwrap(), U256::one());

        //returns input price if amount in is zero and zeroForOne = true
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e17 as u128,
            U256::zero(),
            true,
        );

        assert_eq!(result.unwrap(), U256::from_dec_str("79228162514264337593543950336").unwrap());

        //returns input price if amount in is zero and zeroForOne = false
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e17 as u128,
            U256::zero(),
            true,
        );

        assert_eq!(result.unwrap(), U256::from_dec_str("79228162514264337593543950336").unwrap());

        //returns the minimum price for max inputs

        let sqrt_price = MAX_U160;
        let liquidity = u128::MAX;
        let max_amount_no_overflow = U256::MAX - ((U256::from(liquidity) << 96) / sqrt_price);
        let result =
            get_next_sqrt_price_from_input(sqrt_price, liquidity, max_amount_no_overflow, true);
        assert_eq!(result.unwrap(), U256::one());

        //input amount of 0.1 token1
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e18 as u128,
            U256::from_dec_str("100000000000000000").unwrap(),
            false,
        );

        assert_eq!(result.unwrap(), U256::from_dec_str("87150978765690771352898345369").unwrap());

        //input amount of 0.1 token0
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e18 as u128,
            U256::from_dec_str("100000000000000000").unwrap(),
            true,
        );

        assert_eq!(result.unwrap(), U256::from_dec_str("72025602285694852357767227579").unwrap());

        //amountIn > type(uint96).max and zeroForOne = true
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e19 as u128,
            U256::from_dec_str("1267650600228229401496703205376").unwrap(),
            true,
        );
        // perfect answer:
        // https://www.wolframalpha.com/input/?i=624999999995069620+-+%28%281e19+*+1+%2F+%281e19+%2B+2%5E100+*+1%29%29+*+2%5E96%29
        assert_eq!(result.unwrap(), U256::from_dec_str("624999999995069620").unwrap());

        //can return 1 with enough amountIn and zeroForOne = true
        let result = get_next_sqrt_price_from_input(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1,
            U256::MAX / 2,
            true,
        );

        assert_eq!(result.unwrap(), U256::one());
    }

    #[test]
    fn test_get_next_sqrt_price_from_output() {
        //fails if price is zero
        let result =
            get_next_sqrt_price_from_output(U256::zero(), 0, U256::from(1000000000), false);
        assert_eq!(result.unwrap_err().to_string(), "Sqrt price is 0");

        //fails if liquidity is zero
        let result = get_next_sqrt_price_from_output(U256::one(), 0, U256::from(1000000000), false);
        assert_eq!(result.unwrap_err().to_string(), "Liquidity is 0");

        //fails if output amount is exactly the virtual reserves of token0
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(4),
            false,
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "require((product = amount * sqrtPX96) / amount == sqrtPX96 && numerator1 > product);"
        );

        //fails if output amount is greater than virtual reserves of token0
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(5),
            false,
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "require((product = amount * sqrtPX96) / amount == sqrtPX96 && numerator1 > product);"
        );

        //fails if output amount is greater than virtual reserves of token1
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(262145),
            true,
        );
        assert_eq!(result.unwrap_err().to_string(), "Sqrt price is less than or equal to quotient");

        //fails if output amount is exactly the virtual reserves of token1
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(262144),
            true,
        );
        assert_eq!(result.unwrap_err().to_string(), "Sqrt price is less than or equal to quotient");

        //succeeds if output amount is just less than the virtual
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(262143),
            true,
        );
        assert_eq!(result.unwrap(), U256::from_dec_str("77371252455336267181195264").unwrap());

        //puzzling echidna test
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("20282409603651670423947251286016").unwrap(),
            1024,
            U256::from(4),
            false,
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "require((product = amount * sqrtPX96) / amount == sqrtPX96 && numerator1 > product);"
        );

        //returns input price if amount in is zero and zeroForOne = true
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e17 as u128,
            U256::zero(),
            true,
        );
        assert_eq!(result.unwrap(), U256::from_dec_str("79228162514264337593543950336").unwrap());

        //returns input price if amount in is zero and zeroForOne = false
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e17 as u128,
            U256::zero(),
            false,
        );
        assert_eq!(result.unwrap(), U256::from_dec_str("79228162514264337593543950336").unwrap());

        //output amount of 0.1 token1
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e18 as u128,
            U256::from(1e17 as u128),
            false,
        );
        assert_eq!(result.unwrap(), U256::from_dec_str("88031291682515930659493278152").unwrap());

        //output amount of 0.1 token1
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1e18 as u128,
            U256::from(1e17 as u128),
            true,
        );
        assert_eq!(result.unwrap(), U256::from_dec_str("71305346262837903834189555302").unwrap());

        //reverts if amountOut is impossible in zero for one direction
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1,
            U256::MAX,
            true,
        );
        assert_eq!(result.unwrap_err().to_string(), "Denominator is less than or equal to prod_1");

        //reverts if amountOut is impossible in one for zero direction
        let result = get_next_sqrt_price_from_output(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            1,
            U256::MAX,
            false,
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "require((product = amount * sqrtPX96) / amount == sqrtPX96 && numerator1 > product);"
        );
    }

    #[test]
    fn test_get_amount_0_delta() {
        // returns 0 if liquidity is 0
        let amount_0 = _get_amount_0_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            0,
            true,
        );

        assert_eq!(amount_0.unwrap(), U256::zero());

        // returns 0 if prices are equal
        let amount_0 = _get_amount_0_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            0,
            true,
        );

        assert_eq!(amount_0.unwrap(), U256::zero());

        // returns 0.1 amount1 for price of 1 to 1.21
        let amount_0 = _get_amount_0_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            1e18 as u128,
            true,
        )
        .unwrap();

        assert_eq!(amount_0.clone(), U256::from_dec_str("90909090909090910").unwrap());

        let amount_0_rounded_down = _get_amount_0_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            1e18 as u128,
            false,
        );

        assert_eq!(amount_0_rounded_down.unwrap(), amount_0.sub(1));

        // works for prices that overflow
        let amount_0_up = _get_amount_0_delta(
            U256::from_dec_str("2787593149816327892691964784081045188247552").unwrap(),
            U256::from_dec_str("22300745198530623141535718272648361505980416").unwrap(),
            1e18 as u128,
            true,
        )
        .unwrap();

        let amount_0_down = _get_amount_0_delta(
            U256::from_dec_str("2787593149816327892691964784081045188247552").unwrap(),
            U256::from_dec_str("22300745198530623141535718272648361505980416").unwrap(),
            1e18 as u128,
            false,
        )
        .unwrap();

        assert_eq!(amount_0_up, amount_0_down.add(1));
    }

    #[test]
    fn test_get_amount_1_delta() {
        // returns 0 if liquidity is 0
        let amount_1 = _get_amount_1_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            0,
            true,
        );

        assert_eq!(amount_1.unwrap(), U256::zero());

        // returns 0 if prices are equal
        let amount_1 = _get_amount_1_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            0,
            true,
        );

        assert_eq!(amount_1.unwrap(), U256::zero());

        // returns 0.1 amount1 for price of 1 to 1.21
        let amount_1 = _get_amount_1_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            1e18 as u128,
            true,
        )
        .unwrap();

        assert_eq!(amount_1.clone(), U256::from_dec_str("100000000000000000").unwrap());

        let amount_1_rounded_down = _get_amount_1_delta(
            U256::from_dec_str("79228162514264337593543950336").unwrap(),
            U256::from_dec_str("87150978765690771352898345369").unwrap(),
            1e18 as u128,
            false,
        );

        assert_eq!(amount_1_rounded_down.unwrap(), amount_1.sub(1));
    }

    #[test]
    fn test_swap_computation() {
        let sqrt_price =
            U256::from_dec_str("1025574284609383690408304870162715216695788925244").unwrap();
        let liquidity = 50015962439936049619261659728067971248;
        let zero_for_one = true;
        let amount_in = U256::from(406);

        let sqrt_q =
            get_next_sqrt_price_from_input(sqrt_price, liquidity, amount_in, zero_for_one).unwrap();

        assert_eq!(
            sqrt_q,
            U256::from_dec_str("1025574284609383582644711336373707553698163132913").unwrap()
        );

        let amount_0_delta = _get_amount_0_delta(sqrt_q, sqrt_price, liquidity, true).unwrap();

        assert_eq!(amount_0_delta, U256::from(406));
    }
}
