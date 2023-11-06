use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_sol_types::SolInterface;
use brontes_classifier::ActionCollection;
use brontes_classifier::IntoAction;
use brontes_classifier::UniswapV2_Enum;
use brontes_classifier::{StaticBindings, StaticReturnBindings};
use brontes_classifier::{UniswapV2Classifier, PROTOCOL_ADDRESS_MAPPING};
use reth_primitives::{Bytes, H160, H256};
use reth_rpc_types::Log;
use std::default;
use std::str::FromStr;

sol!(UniswapV2, "../crates/brontes-classifier/abis/UniswapV2.json");
sol!(UniswapV3, "../crates/brontes-classifier/abis/UniswapV3.json");

#[test]
fn test_decode() {
    let sig: &[u8] = &[2, 44, 13, 159];
    let index = 2;
    let calldata = Bytes::from_str("0x022c0d9f000000000000000000000000000000000000000000000000000065c3241b7c590000000000000000000000000000000000000000000000000000000000000000000000000000000000000000cc2687c14915fd68226ccf388842515739a739bd00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let return_bytes = Bytes::default();
    let from_address = H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap();
    let target_address = H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    println!(
        "{:?}",
        Bytes::from(
            UniswapV2::UniswapV2Calls::abi_decode(&calldata, true)
                .unwrap()
                .abi_encode()
                .as_slice()
        )
    );


    let logs = vec![
    Log { 
        address: H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(), 
        topics: vec![H256::from_str("0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1").unwrap()], 
        data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000006a5a6631a0b350000000000000000000000000000000000000000000000006f88a8f5e7e80584").unwrap(), 
        block_hash: Some(H256::from_str("0x5e27d41148af7d2a4aca473c516223fe30bbe1b32f17c023b3c89c2be6d6e98d").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011101d8").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
     Log { 
        address: H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap(), 
        topics: vec![
            H256::from_str("0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822").unwrap(),
            H256::from_str("0x000000000000000000000000cc2687c14915fd68226ccf388842515739a739bd").unwrap(),
            H256::from_str("0x000000000000000000000000cc2687c14915fd68226ccf388842515739a739bd").unwrap()
        ],
        data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064fbb84aac0dc8e000000000000000000000000000000000000000000000000000065c3241b7c590000000000000000000000000000000000000000000000000000000000000000").unwrap(), 
        block_hash: Some(H256::from_str("0x5e27d41148af7d2a4aca473c516223fe30bbe1b32f17c023b3c89c2be6d6e98d").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011101d8").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000003").unwrap().into()),
        removed: false
    }

    ];
    //UniswapV2::UniswapV2Calls::swap(())  abi_decode(&calldata, true).unwrap();
    //UniswapV2::swapCall::de;
    let classifier = UniswapV2Classifier::default();
    println!("{:?}", classifier.0);
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);
    //println!("call data decoded: {:?}", UniswapV2::abi_decode(calldata));
    println!("dispatch: {:?}", res);
}
