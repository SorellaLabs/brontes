use alloy_primitives::U256;

const PRECISION: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

const IN_TOKEN_AMOUNT: U256 = U256::from_limbs([1, 0, 0, 0]);

pub fn calculate_exchange_rate_stable(
    vp_rate: U256,
    balances: Vec<U256>,
    rates: Vec<U256>,
    n_assets: U256,
    sold_id: usize,
    bought_id: usize,
    amp: U256,
) -> U256 {
    let rates_clone = rates.clone();
    let xp = xp_mem(vp_rate, balances, rates, n_assets);
    let x = xp[sold_id] + IN_TOKEN_AMOUNT * (rates_clone[sold_id] / PRECISION);
    let y = get_y(sold_id, bought_id, x, xp.clone(), amp, n_assets);
    xp[bought_id] - y - U256::from(1)
}

pub fn get_d(xp: Vec<U256>, amp: U256, n_assets: U256) -> U256 {
    let s: U256 = xp.iter().sum();
    if s == U256::ZERO {
        return U256::ZERO;
    }

    let mut d = s;
    let ann = amp * U256::from(xp.len());
    let a_precision = U256::from(100);

    for _ in 0..255 {
        let d_p: U256 = xp.iter().fold(d, |acc, &x| (acc * d) / (x * n_assets));
        let d_prev = d;
        d = (((ann * s) / a_precision + d_p * n_assets) * d)
            / (((ann - a_precision) * d) / a_precision + (n_assets + U256::from(1)) * d_p);

        if (d > d_prev && d - d_prev <= U256::from(1))
            || (d <= d_prev && d_prev - d <= U256::from(1))
        {
            break;
        }
    }
    d
}

pub fn get_y(i: usize, j: usize, x: U256, xp_: Vec<U256>, amp: U256, n_assets: U256) -> U256 {
    let n_coins = n_assets;
    assert!(i != j, "same coin");
    assert!(U256::from(j) < n_coins, "j above N_COINS");
    assert!(U256::from(i) < n_coins, "i above N_COINS");

    let d: U256 = get_d(xp_.clone(), amp, n_coins);
    let a_precision = U256::from(100);
    let mut s_: U256 = U256::ZERO;
    let mut c: U256 = d;
    let ann: U256 = amp * U256::from(n_coins);

    for (_i, _x) in xp_.iter().enumerate() {
        let x_val = if _i == i {
            x
        } else if _i != j {
            *_x
        } else {
            continue;
        };
        s_ += x_val;
        c = (c * d) / (x_val * U256::from(n_coins));
    }

    c = (c * d * a_precision) / (ann * U256::from(n_coins));
    let b: U256 = s_ + (d * a_precision) / ann;
    let mut y: U256 = d;

    for _ in 0..255 {
        let y_prev = y;
        y = (y * y + c) / (U256::from(2) * y + b - d);
        if (y > y_prev && y - y_prev <= U256::from(1))
            || (y <= y_prev && y_prev - y <= U256::from(1))
        {
            break;
        }
    }
    y
}

pub fn xp_mem(vp_rate: U256, balance: Vec<U256>, rates: Vec<U256>, n_assets: U256) -> Vec<U256> {
    let n_assets_usize = n_assets.to();
    assert!(n_assets_usize <= rates.len(), "n_assets exceeds rates length");
    assert!(n_assets_usize <= balance.len(), "n_assets exceeds balance length");
    assert!(n_assets_usize > 0, "n_assets must be greater than zero");

    let mut result = rates;

    result[n_assets_usize - 1] = vp_rate;

    for i in 0..n_assets_usize {
        result[i] = result[i] * balance[i] / PRECISION;
    }

    result
}
