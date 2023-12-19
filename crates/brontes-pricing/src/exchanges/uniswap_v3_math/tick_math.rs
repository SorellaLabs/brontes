use std::ops::{BitOr, Neg, Shl, Shr};

use alloy_primitives::{I256, U256};

use super::error::UniswapV3MathError;

pub const MIN_TICK: i32 = -887272;
pub const MAX_TICK: i32 = -MIN_TICK;

pub const MIN_SQRT_RATIO: U256 = U256::from_limbs([4295128739, 0, 0, 0]);
pub const MAX_SQRT_RATIO: U256 =
    U256::from_limbs([6743328256752651558, 17280870778742802505, 4294805859, 0]);

pub const SQRT_10001: I256 = I256::from_raw(U256::from_limbs([11745905768312294533, 13863, 0, 0]));
pub const TICK_LOW: I256 =
    I256::from_raw(U256::from_limbs([6552757943157144234, 184476617836266586, 0, 0]));
pub const TICK_HIGH: I256 =
    I256::from_raw(U256::from_limbs([4998474450511881007, 15793544031827761793, 0, 0]));

pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, UniswapV3MathError> {
    let abs_tick =
        if tick < 0 { U256::from_be_bytes(tick.neg().to_be_bytes()) } else { U256::from(tick) };

    if abs_tick > U256::from(MAX_TICK) {
        return Err(UniswapV3MathError::T)
    }

    let mut ratio = if abs_tick & (U256::from(0x1)) != U256::ZERO {
        U256::from(0xfffcb933bd6fad37aa2d162d1a594001 as u128)
    } else {
        U256::from_str_radix("100000000000000000000000000000000", 16)
            .map_err(|_| UniswapV3MathError::T)?
    };

    if !(abs_tick & (U256::from(0x2))).is_zero() {
        ratio = (ratio * U256::from(0xfff97272373d413259a46990580e213a as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x4))).is_zero() {
        ratio = (ratio * U256::from(0xfff2e50f5f656932ef12357cf3c7fdcc as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x8))).is_zero() {
        ratio = (ratio * U256::from(0xffe5caca7e10e4e61c3624eaa0941cd0 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x10))).is_zero() {
        ratio = (ratio * U256::from(0xffcb9843d60f6159c9db58835c926644 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x20))).is_zero() {
        ratio = (ratio * U256::from(0xff973b41fa98c081472e6896dfb254c0 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x40))).is_zero() {
        ratio = (ratio * U256::from(0xff2ea16466c96a3843ec78b326b52861 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x80))).is_zero() {
        ratio = (ratio * U256::from(0xfe5dee046a99a2a811c461f1969c3053 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x100))).is_zero() {
        ratio = (ratio * U256::from(0xfcbe86c7900a88aedcffc83b479aa3a4 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x200))).is_zero() {
        ratio = (ratio * U256::from(0xf987a7253ac413176f2b074cf7815e54 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x400))).is_zero() {
        ratio = (ratio * U256::from(0xf3392b0822b70005940c7a398e4b70f3 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x800))).is_zero() {
        ratio = (ratio * U256::from(0xe7159475a2c29b7443b29c7fa6e889d9 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x1000))).is_zero() {
        ratio = (ratio * U256::from(0xd097f3bdfd2022b8845ad8f792aa5825 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x2000))).is_zero() {
        ratio = (ratio * U256::from(0xa9f746462d870fdf8a65dc1f90e061e5 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x4000))).is_zero() {
        ratio = (ratio * U256::from(0x70d869a156d2a1b890bb3df62baf32f7 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x8000))).is_zero() {
        ratio = (ratio * U256::from(0x31be135f97d08fd981231505542fcfa6 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x10000))).is_zero() {
        ratio = (ratio * U256::from(0x9aa508b5b7a84e1c677de54f3e99bc9 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x20000))).is_zero() {
        ratio = (ratio * U256::from(0x5d6af8dedb81196699c329225ee604 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x40000))).is_zero() {
        ratio = (ratio * U256::from(0x2216e584f5fa1ea926041bedfe98 as u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x80000))).is_zero() {
        ratio = (ratio * U256::from(0x48a170391f7dc42444e8fa2 as u128)) >> 128
    }

    if tick > 0 {
        ratio = U256::MAX / ratio;
    }

    Ok((ratio >> 32)
        + if (ratio % (U256::from(1) << U256::from(32))).is_zero() {
            U256::ZERO
        } else {
            U256::from(1)
        })
}

pub fn get_tick_at_sqrt_ratio(sqrt_price_x_96: U256) -> Result<i32, UniswapV3MathError> {
    if !(sqrt_price_x_96 >= MIN_SQRT_RATIO && sqrt_price_x_96 < MAX_SQRT_RATIO) {
        return Err(UniswapV3MathError::R)
    }

    let ratio: U256 = sqrt_price_x_96.shl(32);
    let mut r = ratio;
    let mut msb = U256::ZERO;

    let mut f: U256 = if r > U256::from(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF as u128) {
        U256::from(1).shl(U256::from(7))
    } else {
        U256::ZERO
    };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFFFFFFFFFFFFFFFF as u128) {
        U256::from(1).shl(U256::from(6))
    } else {
        U256::ZERO
    };

    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFFFFFFFF as u128) {
        U256::from(1).shl(U256::from(5))
    } else {
        U256::ZERO
    };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFFFF) { U256::from(1).shl(U256::from(4)) } else { U256::ZERO };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFF) { U256::from(1).shl(U256::from(3)) } else { U256::ZERO };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xF) { U256::from(1).shl(U256::from(2)) } else { U256::ZERO };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0x3) { U256::from(1).shl(U256::from(1)) } else { U256::ZERO };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0x1) { U256::from(1) } else { U256::ZERO };

    msb = msb.bitor(f);

    r = if msb >= U256::from(128) {
        ratio.shr(msb - U256::from(127))
    } else {
        ratio.shl(U256::from(127) - msb)
    };

    let mut log_2: I256 = (I256::from_raw(msb) - I256::from_raw(U256::from(128))).shl(64);

    for i in (51..=63).rev() {
        r = r.overflowing_mul(r).0.shr(U256::from(127));
        let f: U256 = r.shr(128);
        log_2 = log_2.bitor(I256::from_raw(f.shl(i)));

        r = r.shr(f);
    }

    r = r.overflowing_mul(r).0.shr(U256::from(127));
    let f: U256 = r.shr(128);
    log_2 = log_2.bitor(I256::from_raw(f.shl(50)));

    let log_sqrt10001 = log_2.wrapping_mul(SQRT_10001);

    let tick_low = ((log_sqrt10001 - TICK_LOW) >> 128_u8).low_i32();

    let tick_high = ((log_sqrt10001 + TICK_HIGH) >> 128_u8).low_i32();

    let tick = if tick_low == tick_high {
        tick_low
    } else if get_sqrt_ratio_at_tick(tick_high)? <= sqrt_price_x_96 {
        tick_high
    } else {
        tick_low
    };

    Ok(tick)
}

#[cfg(test)]
mod test {
    use std::ops::Sub;

    use super::*;

    #[test]
    fn get_sqrt_ratio_at_tick_bounds() {
        // the function should return an error if the tick is out of bounds
        if let Err(err) = get_sqrt_ratio_at_tick(MIN_TICK - 1) {
            assert!(matches!(err, UniswapV3MathError::T));
        } else {
            panic!("get_qrt_ratio_at_tick did not respect lower tick bound")
        }
        if let Err(err) = get_sqrt_ratio_at_tick(MAX_TICK + 1) {
            assert!(matches!(err, UniswapV3MathError::T));
        } else {
            panic!("get_qrt_ratio_at_tick did not respect upper tick bound")
        }
    }

    #[test]
    fn get_sqrt_ratio_at_tick_values() {
        // test individual values for correct results
        assert_eq!(
            get_sqrt_ratio_at_tick(MIN_TICK).unwrap(),
            4295128739u64.into(),
            "sqrt ratio at min incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(MIN_TICK + 1).unwrap(),
            4295343490u64.into(),
            "sqrt ratio at min + 1 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(MAX_TICK - 1).unwrap(),
            U256::from_dec_str("1461373636630004318706518188784493106690254656249").unwrap(),
            "sqrt ratio at max - 1 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(MAX_TICK).unwrap(),
            U256::from_dec_str("1461446703485210103287273052203988822378723970342").unwrap(),
            "sqrt ratio at max incorrect"
        );
        // checking hard coded values against solidity results
        assert_eq!(
            get_sqrt_ratio_at_tick(50).unwrap(),
            79426470787362580746886972461u128.into(),
            "sqrt ratio at 50 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(100).unwrap(),
            79625275426524748796330556128u128.into(),
            "sqrt ratio at 100 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(250).unwrap(),
            80224679980005306637834519095u128.into(),
            "sqrt ratio at 250 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(500).unwrap(),
            81233731461783161732293370115u128.into(),
            "sqrt ratio at 500 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(1000).unwrap(),
            83290069058676223003182343270u128.into(),
            "sqrt ratio at 1000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(2500).unwrap(),
            89776708723587163891445672585u128.into(),
            "sqrt ratio at 2500 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(3000).unwrap(),
            92049301871182272007977902845u128.into(),
            "sqrt ratio at 3000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(4000).unwrap(),
            96768528593268422080558758223u128.into(),
            "sqrt ratio at 4000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(5000).unwrap(),
            101729702841318637793976746270u128.into(),
            "sqrt ratio at 5000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(50000).unwrap(),
            965075977353221155028623082916u128.into(),
            "sqrt ratio at 50000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(150000).unwrap(),
            143194173941309278083010301478497u128.into(),
            "sqrt ratio at 150000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(250000).unwrap(),
            21246587762933397357449903968194344u128.into(),
            "sqrt ratio at 250000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(500000).unwrap(),
            U256::from_dec_str("5697689776495288729098254600827762987878").unwrap(),
            "sqrt ratio at 500000 incorrect"
        );
        assert_eq!(
            get_sqrt_ratio_at_tick(738203).unwrap(),
            U256::from_dec_str("847134979253254120489401328389043031315994541").unwrap(),
            "sqrt ratio at 738203 incorrect"
        );
    }

    #[test]
    pub fn test_get_tick_at_sqrt_ratio() {
        //throws for too low
        let result = get_tick_at_sqrt_ratio(MIN_SQRT_RATIO.sub(1));
        assert_eq!(
            result.unwrap_err().to_string(),
            "Second inequality must be < because the price can never reach the price at the max \
             tick"
        );

        //throws for too high
        let result = get_tick_at_sqrt_ratio(MAX_SQRT_RATIO);
        assert_eq!(
            result.unwrap_err().to_string(),
            "Second inequality must be < because the price can never reach the price at the max \
             tick"
        );

        //ratio of min tick
        let result = get_tick_at_sqrt_ratio(MIN_SQRT_RATIO).unwrap();
        assert_eq!(result, MIN_TICK);

        //ratio of min tick + 1
        let result = get_tick_at_sqrt_ratio(U256::from_dec_str("4295343490").unwrap()).unwrap();
        assert_eq!(result, MIN_TICK + 1);
    }
}
