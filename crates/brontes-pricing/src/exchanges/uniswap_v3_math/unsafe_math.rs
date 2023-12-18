use alloy_primitives::U256;

pub fn div_rounding_up(a: U256, b: U256) -> U256 {
    let (quotient, remainder) = a.div_mod(b);
    if remainder.is_zero() {
        quotient
    } else {
        quotient + 1
    }
}
