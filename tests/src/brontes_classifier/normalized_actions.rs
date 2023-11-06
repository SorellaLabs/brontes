use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_sol_types::SolEvent;
use alloy_sol_types::SolInterface;
use brontes_classifier::ActionCollection;
use brontes_classifier::IntoAction;
use brontes_classifier::UniswapV2_Enum;
use brontes_classifier::{StaticBindings, StaticReturnBindings};
use brontes_classifier::{UniswapV2Classifier, PROTOCOL_ADDRESS_MAPPING};
use brontes_types::normalized_actions::Actions;
use brontes_types::normalized_actions::NormalizedBurn;
use brontes_types::normalized_actions::NormalizedMint;
use brontes_types::normalized_actions::NormalizedSwap;
use reth_primitives::{Bytes, H160, H256};
use reth_rpc_types::Log;
use std::default;
use std::str::FromStr;

use crate::brontes_classifier::UniswapV2;
use crate::brontes_classifier::UniswapV3;

#[test]
fn test_uni_v2_burn() {
    let classifier = UniswapV2Classifier::default();

    let sig: &[u8] = &UniswapV2::burnCall::SELECTOR;
    let index = 96;
    let calldata = Bytes::from_str("0x6a6278420000000000000000000000004d047bcb94f45bd745290333d2c9bdedc94f36e5").unwrap();
    //println!("{:?}", UniswapV2::mintCall::abi_decode(&calldata, true).unwrap().to);
    let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let from_address = H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();
    let target_address = H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap(), 
        topics: vec![H256::from_str("0x4c209b5fc8ad50758f13e2e1088ba56a560dff690a1c6fef26394f4c03821c4f").unwrap().into(), H256::from_str("0x0000000000000000000000007a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap(), H256::from_str("0x0000000000000000000000007a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()], 
        data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000039e198d98cdedd0000000000000000000000000000000000000000000000000000000001d41eab").unwrap(), 
        block_hash: Some(H256::from_str("0x560ec4a3a96b26a86a5f779384bc3f4249f22afb8a116f20ce224766305f5e99").unwrap()), 
        block_number: Some(H256::from_str("0x000000000000000000000000000000000000000000000000000000000119371A").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xd42987b923b9e10de70df67b2bb57eefe21dec0a4c0372d3bcbdb69feb34dff4").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000060").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
    ];

    println!("res amt0: {:?}", UniswapV2::Mint::decode_log(logs[0].topics.iter().map(|h| h.0), &logs[0].data, true).unwrap().amount0);
    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_mint());

    let burn = match action {
        Actions::Burn(s) => s,
        _ => unreachable!()
    };

    let expected_burn = NormalizedBurn {
        index: 96,
        from: from_address,
        token:  vec![H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(), H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap()],
        to: target_address,
        recipient: target_address,
        amount: vec![H256::from_low_u64_be(999999641424138).into(), H256::from_low_u64_be(1792743).into()],
    };

    assert_eq!(burn, expected_burn);
}


#[test]
fn test_uni_v2_mint() {
    let classifier = UniswapV2Classifier::default();

    let sig: &[u8] = &UniswapV2::mintCall::SELECTOR;
    let index = 96;
    let calldata = Bytes::from_str("0x6a6278420000000000000000000000004d047bcb94f45bd745290333d2c9bdedc94f36e5").unwrap();
    let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let from_address = H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();
    let target_address = H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap(), 
        topics: vec![H256::from_str("0x4c209b5fc8ad50758f13e2e1088ba56a560dff690a1c6fef26394f4c03821c4f").unwrap().into(), H256::from_str("0x0000000000000000000000007a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()], 
        data: Bytes::from_str("0x00000000000000000000000000000000000000000000000000038d7e8f67110a00000000000000000000000000000000000000000000000000000000001b5ae7").unwrap(), 
        block_hash: Some(H256::from_str("0x560ec4a3a96b26a86a5f779384bc3f4249f22afb8a116f20ce224766305f5e99").unwrap()), 
        block_number: Some(H256::from_str("0x000000000000000000000000000000000000000000000000000000000119371A").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xd42987b923b9e10de70df67b2bb57eefe21dec0a4c0372d3bcbdb69feb34dff4").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000060").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
    ];

    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_mint());

    let mint = match action {
        Actions::Mint(s) => s,
        _ => unreachable!()
    };

    let expected_mint = NormalizedMint {
        index: 96,
        from: from_address,
        token:  vec![H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(), H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap()],
        to: target_address,
        recipient: target_address,
        amount: vec![H256::from_low_u64_be(999999641424138).into(), H256::from_low_u64_be(1792743).into()],
    };

    assert_eq!(mint, expected_mint);
}

#[test]
fn test_uni_v2_swap() {
    let classifier = UniswapV2Classifier::default();

    let sig: &[u8] = &UniswapV2::swapCall::SELECTOR;
    let index = 2;
    let calldata = Bytes::from_str("0x022c0d9f000000000000000000000000000000000000000000000000000065c3241b7c590000000000000000000000000000000000000000000000000000000000000000000000000000000000000000cc2687c14915fd68226ccf388842515739a739bd00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let return_bytes = Bytes::default();
    let from_address = H160::from_str("0xcc2687c14915fd68226ccf388842515739a739bd").unwrap();
    let target_address = H160::from_str("0xde55ec8002d6a3480be27e0b9755ef987ad6e151").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

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
    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_swap());

    let swap = match action {
        Actions::Swap(s) => s,
        _ => unreachable!()
    };

    let expected_swap = NormalizedSwap {
        index: 2,
        from: from_address,
        pool: target_address,
        token_in: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
        token_out:  H160::from_str("0x728b3f6a79f226bc2108d21abd9b455d679ef725").unwrap(),
        amount_in: H256::from_low_u64_be(454788265862552718).into(),
        amount_out: H256::from_low_u64_be(111888798809177).into(),
    };

    assert_eq!(swap, expected_swap);
}
