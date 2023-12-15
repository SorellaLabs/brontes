use alloy_primitives::{Address, U256};
use alloy_rpc_types::{state::AccountOverride, CallRequest};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::{try_get_decimals, ToScaledRational};
use malachite::{
    num::arithmetic::traits::{Reciprocal, ReciprocalAssign},
    Rational,
};
use reth_rpc_types::trace::parity::{ChangedType, Delta, StateDiff};

use crate::{
    decoding::TracingProvider,
    dex_price::{into_state_overrides, make_call_request},
};

sol!(
    interface IUniswapV3 {
        function token0() external view returns (address);

        /// @notice The second of the two tokens of the pool, sorted by address
        /// @return The token contract address
        function token1() external view returns (address);

        function slot0()
            external
            view
            returns (
                uint160 sqrtPriceX96,
                int24 tick,
                uint16 observationIndex,
                uint16 observationCardinality,
                uint16 observationCardinalityNext,
                uint8 feeProtocol,
                bool unlocked
            );
    }
);

sol! (
    function balanceOf(address owner) external view returns (uint);
);

#[derive(Default)]
struct V3Pricing;

impl DexPrice for V3Pricing {
    fn get_price<T: TracingProvider>(
        &self,
        provider: Arc<T>,
        block: u64,
        address: Address,
        zto: bool,
        state_diff: StateDiff,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>> {
        Box::pin(async {
            let diff = into_state_overrides(state_diff);
            let slot0 = make_call_request(
                IUniswapV3::slot0Call::new(()),
                provider.clone(),
                Some(diff.clone()),
                address,
                block,
            )
            .await;

            let token0 = make_call_request(
                IUniswapV3::token0Call::new(()),
                provider.clone(),
                None,
                address,
                block,
            )
            .await;
            let token1 = make_call_request(
                IUniswapV3::token1Call::new(()),
                provider.clone(),
                None,
                address,
                block,
            )
            .await;

            let dec0 = try_get_decimals(&**token0._0).unwrap();
            let dec1 = try_get_decimals(&**token1._0).unwrap();

            let balance0 = make_call_request(
                balanceOfCall::new((address)),
                provider.clone(),
                Some(diff.clone()),
                token0._0,
                block,
            )
            .await;

            let balance1 = make_call_request(
                balanceOfCall::new((address)),
                provider.clone(),
                Some(diff.clone()),
                token1._0,
                block,
            )
            .await;

            let sqrt = slot0.sqrtPriceX96.to::<U256>().to_scaled_rational(0);
            let ratio: Rational = (sqrt / Rational::from(Integer::from(2).pow(96))).pow(2u64);

            let mut price: Rational = ratio
                / Rational::from(
                    Integer::from(10).pow(tokens_reserves.0.decimals - tokens_reserves.1.decimals),
                );

            if !zto {
                first_price.reciprocal_assign();
            }

            (price, balance1._0.to_scaled_rational(0) * balance0._0.to_scaled_rational(0))
        })
    }
}
