use std::{collections::HashMap, ops::MulAssign};

use malachite::{num::arithmetic::traits::ReciprocalAssign, Rational};

use crate::{DexQuotesMap, Pair, Quote};

#[derive(Debug, Clone, Default)]
pub struct DexQuote(pub HashMap<usize, Rational>);

impl Quote for DexQuote {
    fn inverse_price(&mut self) {
        for v in self.0.values_mut() {
            v.reciprocal_assign()
        }
    }
}

impl DexQuote {
    pub fn get_price(&self, block_position: usize) -> Rational {
        self.0.get(&block_position).unwrap().clone()
    }
}

impl MulAssign for DexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        assert!(self.0.len() == rhs.0.len(), "rhs.len() != lhs.len()");

        for (k, v) in rhs.0 {
            *self.0.get_mut(&k).unwrap() *= v;
        }
    }
}

impl From<HashMap<usize, HashMap<Pair, Rational>>> for DexQuotesMap<DexQuote> {
    fn from(value: HashMap<usize, HashMap<Pair, Rational>>) -> Self {
        todo!()
    }
}
