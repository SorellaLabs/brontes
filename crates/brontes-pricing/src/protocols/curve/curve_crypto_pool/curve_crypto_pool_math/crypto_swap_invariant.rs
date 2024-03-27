use alloy_primitives::U256;

const PRECISION: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);
const A_MULTIPLIER: U256 = U256::from_limbs([10000, 0, 0, 0]);
const U256_ONE: U256 = U256::from_limbs([1, 0, 0, 0]);
const MIN_GAMMA: U256 = U256::from_limbs([1_000_000_000_0, 0, 0, 0]);
const MAX_GAMMA: U256 = U256::from_limbs([5_000_000_000_000_000_0, 0, 0, 0]);

pub fn calculate_exchange_rate_crypto(
    reserves: Vec<U256>,
    n_assets: U256,
    coin_decimals: Vec<u8>,
    sold_id: usize,
    bought_id: usize,
    price_scale_packed: U256,
    future_a_gamma_time: Option<usize>,
    d_value: U256,
    a_0: U256,
    a_1: U256,
) -> U256 {
    let mut xp = reserves;
    let price_size = U256::from(256) / (n_assets - U256::from(1));
    let price_mask = U256::from(2).pow(price_size) - U256::from(1);
    let mut price_scale = Vec::new();
    let mut packed_prices = price_scale_packed;
    let n_coins: usize = n_assets.to();
    let mut d_var = d_value;
    for _ in 0..n_coins - 1 {
        let price = packed_prices & price_mask; // Extract price using bitwise AND
        price_scale.push(price); // Store the extracted price
        packed_prices = packed_prices >> price_size; // Right shift for next price
    }
    let mut precisions = Vec::new();
    for i in 0..n_coins {
        let precision = U256::from(10).pow(U256::from(18 - coin_decimals[i]));
        precisions.push(precision);
    }
    xp[0] *= precisions[0];

    for k in 1..n_coins {
        xp[k] = (xp[k] * price_scale[k - 1] * precisions[k]) / PRECISION;
    }
    let prec_i = precisions[sold_id];
    if let Some(_future_a_gamma_time) = future_a_gamma_time {
        let mut x0 = xp[sold_id];
        x0 *= prec_i;
        if sold_id > 0 {
            x0 = (x0 * price_scale[sold_id - 1]) / PRECISION;
        }
        let x1 = xp[sold_id];
        xp[sold_id] = x0;
        d_var = newton_d( a_0, a_1, &xp, n_assets);
        xp[sold_id] = x1;
    }

    let prec_j = precisions[bought_id];
    let mut dy: U256 =
        xp[bought_id] -
        newton_y(a_0, a_1, xp, d_var, bought_id, n_assets);
    dy -= U256::from(1);
    if bought_id > 0 {
        dy = (dy * PRECISION) / price_scale[bought_id - 1];
    }
    dy / prec_j
}

fn newton_d(ann: U256, gamma: U256, x_unsorted: &Vec<U256>, n_assets: U256) -> U256 {
    let n_coins = n_assets.clone();
    let min_a = (n_coins.pow(n_coins) * A_MULTIPLIER) / U256::from(100);
    let max_a = n_coins.pow(n_coins) * A_MULTIPLIER * U256::from(1000);
    assert!(ann > min_a - U256_ONE && ann < max_a + U256_ONE, "unsafe values A");
    assert!(gamma > MIN_GAMMA - U256_ONE && gamma < MAX_GAMMA + U256_ONE, "unsafe values gamma");

    let x = sort(& x_unsorted);
    assert!(
        x[0] > U256::from(1_000_000_000 - 1) &&
            x[0] < U256::from(1_000_000_000_000_000) * PRECISION + U256_ONE,
        "unsafe values x[0]"
    );

    for i in 1..n_coins.to() {
        let frac = (x[i] * PRECISION) / x[0];
        assert!(frac > U256::from(100_000_000_000 - 1), "unsafe values x[i]");
    }

    let mut d = n_coins * geometric_mean(x.clone(), false);
    let mut s = U256::ZERO;
    for x_i in &x {
        s += *x_i;
    }

    for _ in 0..255 {
        let d_prev = d;
        let mut k0 = PRECISION;
        for x_i in &x {
            k0 = (k0 * *x_i * n_coins) / d;
        }

        let mut g1k0 = gamma + PRECISION;
        g1k0 = if g1k0 > k0 { g1k0 - k0 + U256_ONE } else { k0 - g1k0 + U256_ONE };

        let mul1 = (((((PRECISION * d) / gamma) * g1k0) / gamma) * g1k0 * A_MULTIPLIER) / ann;
        let mul2 = (U256::from(2) * PRECISION * n_coins * k0) / g1k0;
        let neg_fprime =
            s + (s * mul2) / PRECISION + (mul1 * n_coins) / k0 - (mul2 * d) / PRECISION;

        let d_plus = (d * (neg_fprime + s)) / neg_fprime;
        let mut d_minus = (d * d) / neg_fprime;
        if PRECISION > k0 {
            d_minus +=
                (((d * (mul1 / neg_fprime)) / PRECISION) * (PRECISION - k0)) / k0;
        } else {
            d_minus -=
                (((d * (mul1 / neg_fprime)) / PRECISION) * (k0 - PRECISION)) / k0;
        }

        d = if d_plus > d_minus { d_plus - d_minus } else { (d_minus - d_plus) / U256::from(2) };

        let diff = if d > d_prev { d - d_prev } else { d_prev - d };
        if diff * (PRECISION / U256::from(10000)) < U256::max(PRECISION / U256::from(100), d) {
            for x_i in &x {
                let frac = (*x_i * PRECISION) / d;
                assert!(
                    frac > (PRECISION / U256::from(100)) - U256_ONE && frac < (PRECISION * U256::from(100)) + U256_ONE,
                    "unsafe values x[i]"
                );
            }
            return d;
        }
    }

    panic!("Did not converge");
}

fn newton_y(ann: U256, gamma: U256, mut x: Vec<U256>, d: U256, i: usize, n_assets: U256) -> U256 {
    let n_coins = n_assets.clone();
    let min_a = (n_coins.pow(n_coins) * A_MULTIPLIER) / U256::from(100);
    let max_a = n_coins.pow(n_coins) * A_MULTIPLIER * U256::from(1000);
    assert!(ann > min_a - U256_ONE && ann < max_a + U256_ONE, "unsafe values A");
    assert!(
        gamma > MIN_GAMMA - U256_ONE && gamma < MAX_GAMMA + U256_ONE,
        "unsafe values gamma"
    );
    assert!(
        d > U256::from(10).pow(U256::from(17)) - U256_ONE &&
            d < U256::from(10).pow(U256::from(15)) * PRECISION + U256_ONE,
        "unsafe values D"
    );

    for &x_k in &x {
        if x_k != x[i] {
            let frac = (x_k * PRECISION) / d;
            assert!(
                frac > (PRECISION / U256::from(100)) - U256_ONE && frac < (PRECISION * U256::from(100)) + U256_ONE,
                "unsafe values x[k]"
            );
        }
    }

    let mut y = d / n_coins;
    let mut k0_i = PRECISION;
    let mut s_i = U256::ZERO;

    x[i] = U256::ZERO; // Temporarily set x[i] to 0 for calculations
    let x_sorted = sort(&x);
    let asset_count: usize = n_coins.to();
    for j in 2..=asset_count {
        let _x = x_sorted[asset_count - j];
        y = (y * d) / (_x * n_coins);
        s_i += _x;
    }
    for x_j in &x_sorted[0..asset_count - 1] {
        k0_i = (k0_i * *x_j * n_coins) / d;
    }

    let convergence_limit = U256::max(
        U256::max(x_sorted[0] / (PRECISION / U256::from(10000)), d / (PRECISION / U256::from(10000))),
        U256::from(100)
    );
    for _ in 0..255 {
        let y_prev = y;
        let k0 = (k0_i * y * n_coins) / d;
        let s = s_i + y;

        let mut g1k0 = gamma + PRECISION;
        g1k0 = if g1k0 > k0 { g1k0 - k0 + U256_ONE } else { k0 - g1k0 + U256_ONE };

        let mul1 = (((((PRECISION * d) / gamma) * g1k0) / gamma) * g1k0 * A_MULTIPLIER) / ann;
        let mul2 = PRECISION + (U256::from(2) * PRECISION * k0) / g1k0;

        let yfprime = PRECISION * y + s * mul2 + mul1;
        let dyfprime = d * mul2;
        let mut y_minus: U256;
        let y_plus: U256;

        if yfprime < dyfprime {
            y = y_prev / U256::from(2);
            continue;
        } else {
            let fprime = yfprime - dyfprime;
            y_minus = mul1 / fprime;
            y_plus = (yfprime + PRECISION * d) / fprime + (y_minus * PRECISION) / k0;
            y_minus += (PRECISION * s) / fprime;
        }

        y = if y_plus > y_minus { y_plus - y_minus } else { y_prev / U256::from(2) }; // Adjust based on condition

        let diff = if y > y_prev { y - y_prev } else { y_prev - y };
        if diff < U256::max(convergence_limit, y / (PRECISION / U256::from(10000))) {
            let frac = (y * PRECISION) / d;
            assert!(
                frac > (PRECISION / U256::from(100)) - U256_ONE && frac < (PRECISION * U256::from(100)) + U256_ONE,
                "unsafe value for y"
            );
            return y;
        }
    }

    panic!("Did not converge");
}

fn sort(values: &Vec<U256>) -> Vec<U256>{
    let mut values_sorted = values.clone();
    values_sorted.sort_by(|a, b| a.cmp(b));
    values_sorted
}

fn geometric_mean(unsorted_x: Vec<U256>, sort: bool) -> U256 {
    let x = if sort {
        let mut sorted_x = unsorted_x.clone();
        sorted_x.sort(); // Sorts in ascending order by default
        sorted_x
    } else {
        unsorted_x
    };

    let n_coins = U256::from(x.len() as u64);
    let mut d = x[0];
    let mut diff :U256;

    for _ in 0..255 {
        let d_prev = d;
        let mut tmp = PRECISION;
        for &_x in &x {
            tmp = tmp.saturating_mul(_x) / d; // Adjusted for U256, using saturating_mul for overflow safety
        }
        d = d.saturating_mul(n_coins.saturating_sub(U256_ONE) * PRECISION + tmp) / (n_coins * PRECISION);
        diff = if d > d_prev { d.saturating_sub(d_prev) } else { d_prev.saturating_sub(d) };

        if diff <= U256_ONE || diff.saturating_mul(PRECISION) < d {
            return d;
        }
    }
    panic!("Did not converge");
}

