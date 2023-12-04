use alloy_primitives::Address;
use reth_primitives::revm_primitives::HashMap;

#[derive(Debug)]
pub struct ExtraProcessing {
    // decimals that are missing that we want to fill
    tokens_decimal_fill: Vec<Address>,
    // dex token prices that we need
    prices:              HashMap<Address, Vec<(Address, Address)>>,
}
