// use std::{pin::Pin, sync::Arc};
//
// use alloy_primitives::Address;
// use alloy_rpc_types::{state::AccountOverride, CallRequest};
// use alloy_sol_macro::sol;
// use alloy_sol_types::SolCall;
// use brontes_types::try_get_decimals;
// use futures::Future;
// use malachite::Rational;
// use reth_rpc_types::trace::parity::{ChangedType, Delta, StateDiff};
//
// use crate::{
//     decoding::TracingProvider,
//     dex_price::{into_state_overrides, make_call_request, DexPrice},
// };
//
// sol!(
//     interface IUniswapV2 {
//         function token0() external view returns (address);
//         function token1() external view returns (address);
//
//         function getReserves() external view returns (
//             uint112 reserve0,
//             uint112 reserve1,
//             uint32 blockTimestampLast
//         );
//     }
// );
//
// #[derive(Default, Clone)]
// struct V2Pricing;
//
// impl DexPrice for V2Pricing {
//     fn get_price<T: TracingProvider>(
//         &self,
//         provider: Arc<T>,
//         block: u64,
//         address: Address,
//         zto: bool,
//         state_diff: StateDiff,
//     ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>> {
//         Box::pin(async {
//             let diff = into_state_overrides(state_diff);
//
//             let reserves = make_call_request(
//                 IUniswapV2::getReservesCall::new(()),
//                 provider.clone(),
//                 Some(diff.clone()),
//                 address,
//                 block,
//             )
//             .await;
//
//             let (r0, r1) = (reserves.reserve0, reserves.reserve1);
//
//             let token0 = make_call_request(
//                 IUniswapV2::token0Call::new(()),
//                 provider.clone(),
//                 None,
//                 address,
//                 block,
//             )
//             .await;
//             let token1 = make_call_request(
//                 IUniswapV2::token1Call::new(()),
//                 provider.clone(),
//                 None,
//                 address,
//                 block,
//             )
//             .await;
//
//             let dec0 = try_get_decimals(&**token0._0).unwrap();
//             let dec1 = try_get_decimals(&**token1._0).unwrap();
//
//             let r0_scaled = r0.to_scaled_rational(dec0);
//             let r1_scaled = r1.to_scaled_rational(dec1);
//
//             let price = if zto { &r1_scaled / &r0_scaled } else { &r0_scaled / &r1_scaled };
//
//             (price, r0_scaled * r1_scaled)
//         })
//     }
// }
