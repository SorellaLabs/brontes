use alloy_primitives::{Uint, U256};

pub const RUINT_ZERO: Uint<256, 4> = Uint::ZERO;
pub const RUINT_ONE: Uint<256, 4> = Uint::<256, 4>::from_limbs([1, 0, 0, 0]);
pub const RUINT_TWO: Uint<256, 4> = Uint::<256, 4>::from_limbs([2, 0, 0, 0]);
pub const RUINT_THREE: Uint<256, 4> = Uint::<256, 4>::from_limbs([3, 0, 0, 0]);
pub const RUINT_MAX_U256: Uint<256, 4> = Uint::<256, 4>::from_limbs([
    18446744073709551615,
    18446744073709551615,
    18446744073709551615,
    18446744073709551615,
]);

