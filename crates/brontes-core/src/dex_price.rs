use alloy_primitives::Address;
use reth_primitives::revm_primitives::{HashMap, Output};

pub trait DexPrice {
    fn get_price(
        &self,
        provider: &Provider<Http<reqwest::Client>>,
        address: Address,
        zto: bool,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>>;
}


// some static map
//"MAP: Map<([u8;20],[u8;20]), Vec<(bool, Address, Box<dyn DexPrice>)>>"

// we will have a static map for (token0, token1) => Vec<address, exchange type>
// this will then process using async, grab the reserves and process the price.
// and return that with tvl. with this we can calculate weighted price
pub struct DexPricing {
    provider: Provider<Http<reqwest::Client>>,
}

impl DexPricing {
    pub fn need_prices_for(&mut self, pools_tokens_type: Vec<(Address, Address)>) {

    }
}
