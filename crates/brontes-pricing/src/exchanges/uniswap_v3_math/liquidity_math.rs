use super::error::UniswapV3MathError;

// returns (uint128 z)
pub fn add_delta(x: u128, y: i128) -> Result<u128, UniswapV3MathError> {
    if y < 0 {
        let z = x.overflowing_sub(-y as u128);

        if z.1 {
            Err(UniswapV3MathError::LiquiditySub)
        } else {
            Ok(z.0)
        }
    } else {
        let z = x.overflowing_add(y as u128);
        if z.0 < x {
            Err(UniswapV3MathError::LiquidityAdd)
        } else {
            Ok(z.0)
        }
    }
}

// #[cfg(test)]
// mod test {
//
//     use crate::liquidity_math::add_delta;
//
//     #[test]
//     fn test_add_delta() {
//         // 1 + 0
//         let result = add_delta(1, 0);
//         assert_eq!(result.unwrap(), 1);
//
//         // 1 + -1
//         let result = add_delta(1, -1);
//         assert_eq!(result.unwrap(), 0);
//
//         // 1 + 1
//         let result = add_delta(1, 1);
//         assert_eq!(result.unwrap(), 2);
//
//         // 2**128-15 + 15 overflows
//         let result = add_delta(340282366920938463463374607431768211441, 15);
//         assert_eq!(result.err().unwrap().to_string(), "Liquidity Add");
//
//         // 0 + -1 underflows
//         let result = add_delta(0, -1);
//         assert_eq!(result.err().unwrap().to_string(), "Liquidity Sub");
//
//         // 3 + -4 underflows
//         let result = add_delta(3, -4);
//         assert_eq!(result.err().unwrap().to_string(), "Liquidity Sub");
//     }
// }
