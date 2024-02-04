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

// NNEED
pub fn get_sqrt_ratio_at_tick(tick: i32) -> Result<U256, UniswapV3MathError> {
    let abs_tick = if tick < 0 {
        U256::from(u32::from_be_bytes(tick.neg().to_be_bytes()))
    } else {
        U256::from(tick)
    };

    if abs_tick > U256::from(MAX_TICK) {
        return Err(UniswapV3MathError::T)
    }

    let mut ratio = if abs_tick & (U256::from(0x1)) != U256::ZERO {
        U256::from(0xfffcb933bd6fad37aa2d162d1a594001_u128)
    } else {
        U256::from_str_radix("100000000000000000000000000000000", 16)
            .map_err(|_| UniswapV3MathError::T)?
    };

    if !(abs_tick & (U256::from(0x2))).is_zero() {
        ratio = (ratio * U256::from(0xfff97272373d413259a46990580e213a_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x4))).is_zero() {
        ratio = (ratio * U256::from(0xfff2e50f5f656932ef12357cf3c7fdcc_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x8))).is_zero() {
        ratio = (ratio * U256::from(0xffe5caca7e10e4e61c3624eaa0941cd0_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x10))).is_zero() {
        ratio = (ratio * U256::from(0xffcb9843d60f6159c9db58835c926644_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x20))).is_zero() {
        ratio = (ratio * U256::from(0xff973b41fa98c081472e6896dfb254c0_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x40))).is_zero() {
        ratio = (ratio * U256::from(0xff2ea16466c96a3843ec78b326b52861_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x80))).is_zero() {
        ratio = (ratio * U256::from(0xfe5dee046a99a2a811c461f1969c3053_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x100))).is_zero() {
        ratio = (ratio * U256::from(0xfcbe86c7900a88aedcffc83b479aa3a4_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x200))).is_zero() {
        ratio = (ratio * U256::from(0xf987a7253ac413176f2b074cf7815e54_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x400))).is_zero() {
        ratio = (ratio * U256::from(0xf3392b0822b70005940c7a398e4b70f3_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x800))).is_zero() {
        ratio = (ratio * U256::from(0xe7159475a2c29b7443b29c7fa6e889d9_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x1000))).is_zero() {
        ratio = (ratio * U256::from(0xd097f3bdfd2022b8845ad8f792aa5825_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x2000))).is_zero() {
        ratio = (ratio * U256::from(0xa9f746462d870fdf8a65dc1f90e061e5_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x4000))).is_zero() {
        ratio = (ratio * U256::from(0x70d869a156d2a1b890bb3df62baf32f7_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x8000))).is_zero() {
        ratio = (ratio * U256::from(0x31be135f97d08fd981231505542fcfa6_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x10000))).is_zero() {
        ratio = (ratio * U256::from(0x9aa508b5b7a84e1c677de54f3e99bc9_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x20000))).is_zero() {
        ratio = (ratio * U256::from(0x5d6af8dedb81196699c329225ee604_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x40000))).is_zero() {
        ratio = (ratio * U256::from(0x2216e584f5fa1ea926041bedfe98_u128)) >> 128
    }
    if !(abs_tick & (U256::from(0x80000))).is_zero() {
        ratio = (ratio * U256::from(0x48a170391f7dc42444e8fa2_u128)) >> 128
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

// NEED
pub fn get_tick_at_sqrt_ratio(sqrt_price_x_96: U256) -> Result<i32, UniswapV3MathError> {
    if !(sqrt_price_x_96 >= MIN_SQRT_RATIO && sqrt_price_x_96 < MAX_SQRT_RATIO) {
        return Err(UniswapV3MathError::R)
    }

    let ratio: U256 = sqrt_price_x_96.shl(32);
    let mut r = ratio;
    let mut msb = U256::ZERO;

    let mut f: U256 = if r > U256::from(0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF_u128) {
        U256::from(1).shl(U256::from(7))
    } else {
        U256::ZERO
    };
    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFFFFFFFFFFFFFFFF_u128) {
        U256::from(1).shl(U256::from(6))
    } else {
        U256::ZERO
    };

    msb = msb.bitor(f);
    r = r.shr(f);

    f = if r > U256::from(0xFFFFFFFF_u128) { U256::from(1).shl(U256::from(5)) } else { U256::ZERO };
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
