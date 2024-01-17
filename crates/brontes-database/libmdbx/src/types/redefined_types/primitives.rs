use alloy_primitives::Uint;
use redefined::{Redefined, RedefinedConvert};

//pub type R_U256 = R_Uint<256, 4>;

#[derive(Redefined)]
#[redefined(Uint, to_source = "Uint::from_limbs(self.limbs)")]
pub struct R_Uint<const BITS: usize, const LIMBS: usize> {
    #[redefined(func = "into_limbs")]
    limbs: [u64; LIMBS],
}

pub type RK = Uint<256, 4>;
