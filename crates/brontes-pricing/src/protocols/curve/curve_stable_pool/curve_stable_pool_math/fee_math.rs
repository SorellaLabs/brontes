use alloy_primitives::U256;

const PRECISION: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

const FEE_DENOMINATOR: U256 = U256::from_limbs([10_000_000_000, 0, 0, 0]);

pub fn calculate_bought_amount_admin_fee(
    rates: Vec<U256>,
    admin_fee: U256,
    fee: U256,
    bought_amount: U256,
    bought_id: usize,
) -> U256 {
    let dy_original = bought_amount * rates[bought_id - 1] / PRECISION;
    let dy_fee = dy_original * fee / FEE_DENOMINATOR;
    dy_fee * admin_fee / FEE_DENOMINATOR
}
