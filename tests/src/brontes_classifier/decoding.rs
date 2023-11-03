use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_sol_types::SolInterface;
use brontes_classifier::ActionCollection;
use brontes_classifier::IntoAction;
use brontes_classifier::StaticBindings;
use brontes_classifier::UniswapV2_Enum;
use brontes_classifier::{UniswapV2Classifier, PROTOCOL_ADDRESS_MAPPING};
use reth_primitives::{Bytes, H160};
use reth_rpc_types::Log;
use std::default;
use std::str::FromStr;

sol!(UniswapV2, "../crates/brontes-classifier/abis/UniswapV2.json");

#[test]
fn test_decode() {
    let sig: &[u8] = &[2, 44, 13, 159];
    let index = 2;
    let calldata = Bytes::from_str("0x022c0d9f000000000000000000000000000000000000000000000000000065c3241b7c590000000000000000000000000000000000000000000000000000000000000000000000000000000000000000cc2687c14915fd68226ccf388842515739a739bd00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let return_bytes = Bytes::default();
    let from_address = H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap();
    let target_address = H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data = bindings.try_decode(&calldata).unwrap();

    UniswapV2::UniswapV2Calls::swap(())  abi_decode(&calldata, true).unwrap();

    let classifier = UniswapV2Classifier::default();
    println!("{:?}", classifier.0);
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &vec![]);
    //println!("call data decoded: {:?}", UniswapV2::abi_decode(calldata));
    println!("dispatch: {:?}", res);
}
