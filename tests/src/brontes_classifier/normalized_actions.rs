use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_sol_types::SolEvent;
use alloy_sol_types::SolInterface;
use brontes_classifier::*;
use brontes_classifier::{StaticBindings, StaticReturnBindings};
use brontes_classifier::{UniswapV2Classifier, PROTOCOL_ADDRESS_MAPPING};
use brontes_types::normalized_actions::*;
use reth_primitives::{Bytes, H160, H256};
use reth_rpc_types::Log;
use std::default;
use std::str::FromStr;

use crate::brontes_classifier::UniswapV2;
use crate::brontes_classifier::UniswapV3;

#[test]
fn test_uni_v3_collect() {
    let classifier = UniswapV3Classifier::default();

    let sig: &[u8] = &UniswapV3::collectCall::SELECTOR;
    println!("SEL: {:?}", Bytes::from(sig));
    let index = 9;
    let calldata = Bytes::from_str("0x4f1eb3d8000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe880000000000000000000000000000000000000000000000000000000000030a0c000000000000000000000000000000000000000000000000000000000003165a0000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap();
    let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap();
    let from_address = H160::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
    let target_address = H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

    let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(),
        topics: vec![
            H256::from_str("0x70935338e69775456a85ddef226c395fb668b63fa0115f5f20610b388e6ca9c0").unwrap().into(),
            H256::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe88").unwrap().into(),
            H256::from_str("0x0000000000000000000000000000000000000000000000000000000000030a0c").unwrap().into(),
            H256::from_str("0x000000000000000000000000000000000000000000000000000000000003165a").unwrap().into(),
        ], 
        data: Bytes::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe880000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap(), 
        block_hash: Some(H256::from_str("0xc698adc7019160c893fd294bae7f30cd78047fe540575c52911bc7ecbc2ab29f").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7F71").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0x9d5cb1e9e83fd5b2c95e5d9f68f48cc90cc5f5b0f44c7a61aa0d260c461e9ebe").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000009").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
    ];

    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_collect());

    let collect = match action {
        Actions::Collect(s) => s,
        _ => unreachable!()
    };

    let expected_collect = NormalizedCollect {
        index: 9,
        from: from_address,
        token:  vec![H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()],
        to: target_address,
        recipient: H160::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap(),
        amount: vec![H256::from_low_u64_be(479371252166).into(), H256::from_str("0x00000000000000000000000000000000000000000000000DE3D53DAD63CDB215").unwrap().into()],
    };

    assert_eq!(collect, expected_collect);
}




#[test]
fn test_uni_v3_burn() {
    let classifier = UniswapV3Classifier::default();

    let sig: &[u8] = &UniswapV3::burnCall::SELECTOR;
    println!("SEL: {:?}", Bytes::from(sig));
    let index = 9;
    let calldata = Bytes::from_str("0xa34123a70000000000000000000000000000000000000000000000000000000000030a0c000000000000000000000000000000000000000000000000000000000003165a00000000000000000000000000000000000000000000000001f18297c89002cb").unwrap();
    let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000000006af143783400000000000000000000000000000000000000000000000d45567f936fa135b8").unwrap();
    let from_address = H160::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
    let target_address = H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

    let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(),
        topics: vec![
            H256::from_str("0x0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c").unwrap().into(),
            H256::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe88").unwrap().into(),
            H256::from_str("0x0000000000000000000000000000000000000000000000000000000000030a0c").unwrap().into(),
            H256::from_str("0x000000000000000000000000000000000000000000000000000000000003165a").unwrap().into(),
        ], 
        data: Bytes::from_str("0x00000000000000000000000000000000000000000000000001f18297c89002cb0000000000000000000000000000000000000000000000000000006af143783400000000000000000000000000000000000000000000000d45567f936fa135b8").unwrap(), 
        block_hash: Some(H256::from_str("0xc698adc7019160c893fd294bae7f30cd78047fe540575c52911bc7ecbc2ab29f").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7F71").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0x9d5cb1e9e83fd5b2c95e5d9f68f48cc90cc5f5b0f44c7a61aa0d260c461e9ebe").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000009").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
    ];

    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_burn());

    let burn = match action {
        Actions::Burn(s) => s,
        _ => unreachable!()
    };

    let expected_burn = NormalizedBurn {
        index: 9,
        from: from_address,
        token:  vec![H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()],
        to: target_address,
        recipient: target_address,
        amount: vec![H256::from_low_u64_be(459314264116).into(), H256::from_str("0x00000000000000000000000000000000000000000000000d45567f936fa135b8").unwrap().into()],
    };

    assert_eq!(burn, expected_burn);
}


#[test]
fn test_uni_v3_mint() {
    let classifier = UniswapV3Classifier::default();

    let sig: &[u8] = &UniswapV3::mintCall::SELECTOR;
    println!("SEL: {:?}", Bytes::from(sig));
    let index = 91;
    let calldata = Bytes::from_str("0x3c8a7d8d000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe880000000000000000000000000000000000000000000000000000000000030c8c00000000000000000000000000000000000000000000000000000000000313a800000000000000000000000000000000000000000000000000082d987468f32000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000080000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000001f400000000000000000000000037164b9a28bb47f17d47ea1338870ac43299d352").unwrap();
    let return_bytes = Bytes::from_str("0x00000000000000000000000000000000000000000000000000000000f8c92fd800000000000000000000000000000000000000000000000022b1c8c115e460a4").unwrap();
    let from_address = H160::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
    let target_address = H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

    let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(), 
        topics: vec![H256::from_str("0x7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde").unwrap().into()], 
        data: Bytes::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe8800000000000000000000000000000000000000000000000000082d987468f32000000000000000000000000000000000000000000000000000000000f8c92fd800000000000000000000000000000000000000000000000022b1c8c115e460a4").unwrap(), 
        block_hash: Some(H256::from_str("0x17790eb817da186ab84c36992090a94ba053b83dbb10619fb04cf006b121eb3e").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7E2B").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xbf46e04f4d44e05064bab5f844aade1fb5d72488c4aaa1fb16103343373daa44").unwrap()), 
        transaction_index: Some(H256::from_str("0x000000000000000000000000000000000000000000000000000000000000005B").unwrap().into()),
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
        index: 91,
        from: from_address,
        token:  vec![H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()],
        to: target_address,
        recipient: H160::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap(),
        amount: vec![H256::from_low_u64_be(4173934552).into(), H256::from_low_u64_be(2499999999788867748).into()],
    };

    assert_eq!(mint, expected_mint);
}



#[test]
fn test_uni_v3_swap() {
    let classifier = UniswapV3Classifier::default();

    let sig: &[u8] = &UniswapV3::swapCall::SELECTOR;
    let index = 150;
    let calldata = Bytes::from_str("0x128acb080000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000006b2c0f3100000000000000000000000000000000000000000000000000000001000276a400000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000400000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000000000002ba0b86991c6218b36c1d19d4a2e9eb0ce3606eb480001f4c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000").unwrap();
    let return_bytes = Bytes::from_str("0x000000000000000000000000000000000000000000000000000000006b2c0f31fffffffffffffffffffffffffffffffffffffffffffffffff2d56e9e92be85db").unwrap();
    let from_address = H160::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap();
    let target_address = H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

    let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
     Log { 
        address: H160::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(), 
        topics: vec![
            H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap(),
            H256::from_str("0x0000000000000000000000003fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD").unwrap(),
            H256::from_str("0x0000000000000000000000003fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD").unwrap()
        ],
        data: Bytes::from_str("0x000000000000000000000000000000000000000000000000000000006b2c0f31fffffffffffffffffffffffffffffffffffffffffffffffff2d56e9e92be85db00000000000000000000000000000000000059c03c850f2ae2fa19a8982682ef0000000000000000000000000000000000000000000000018d27a4400c75f3df0000000000000000000000000000000000000000000000000000000000031096").unwrap(), 
        block_hash: Some(H256::from_str("0x7ceb7355e05f351e82525c7b4e04bc6a41673e071bd9ca9ff33a893721e96a63").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011A8262").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0x681ee84099f113cc13ac4ccc187e702bd64d1f28ef5642e164b405270a012dbd").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000096").unwrap().into()),
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
        index: 150,
        from: from_address,
        pool: target_address,
        token_in: H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        token_out:  H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
        amount_in: H256::from_low_u64_be(1798049585).into(),
        amount_out: H256::from_low_u64_be(948730519145773605).into(),
    };

    assert_eq!(swap, expected_swap);
}





/*

UNI V2 ------------

*/

#[test]
fn test_uni_v2_burn() {
    let classifier = UniswapV2Classifier::default();

    let sig: &[u8] = &UniswapV2::burnCall::SELECTOR;
    let index = 35;
    let calldata = Bytes::from_str("0x89afcb440000000000000000000000007a250d5630b4cf539739df2c5dacb4c659f2488d").unwrap();
    let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000039e198d98cdedd0000000000000000000000000000000000000000000000000000000001d41eab").unwrap();
    let from_address = H160::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();
    let target_address = H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap();

    let bindings = StaticBindings::UniswapV2(UniswapV2_Enum::None);
    let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

    let logs = vec![
    Log { 
        address: H160::from_str("0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852").unwrap(), 
        topics: vec![H256::from_str("0xdccd412f0b1252819cb1fd330b93224ca42612892bb3f4f789976e6d81936496").unwrap().into(), H256::from_str("0x0000000000000000000000007a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap(), H256::from_str("0x0000000000000000000000007a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()], 
        data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000039e198d98cdedd0000000000000000000000000000000000000000000000000000000001d41eab").unwrap(), 
        block_hash: Some(H256::from_str("0x560ec4a3a96b26a86a5f779384bc3f4249f22afb8a116f20ce224766305f5e99").unwrap()), 
        block_number: Some(H256::from_str("0x00000000000000000000000000000000000000000000000000000000011A57BF").unwrap().into()), 
        transaction_hash: Some(H256::from_str("0xbc2b4d335d7d8280546edd824f2ae0b8435a67b5572839f1d5dc45ce675df769").unwrap()), 
        transaction_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000023").unwrap().into()),
        log_index: Some(H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
        removed: false
     },
    ];
    
    let res =
        classifier.dispatch(sig, index, data, return_bytes, from_address, target_address, &logs);

    assert!(res.is_some());

    let action = res.unwrap();
    assert!(action.is_burn());

    let burn = match action {
        Actions::Burn(s) => s,
        _ => unreachable!()
    };

    let expected_burn = NormalizedBurn {
        index: 35,
        from: from_address,
        token:  vec![H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(), H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap()],
        to: target_address,
        recipient:H160::from_str("0x7a250d5630b4cf539739df2c5dacb4c659f2488d").unwrap(),
        amount: vec![H256::from_low_u64_be(16292120273673949).into(), H256::from_low_u64_be(30678699).into()],
    };

    assert_eq!(burn, expected_burn);
}


#[test]
fn test_uni_v2_mint() {
    let classifier = UniswapV2Classifier::default();

    let sig: &[u8] = &UniswapV2::mintCall::SELECTOR;
    let index = 96;
    let calldata = Bytes::from_str("0x6a6278420000000000000000000000004d047bcb94f45bd745290333d2c9bdedc94f36e5").unwrap();
    let return_bytes = Bytes::from_str("0x00000000000000000000000000000000000000000000000000000004292ca7a9").unwrap();
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
        recipient: H160::from_str("0x4d047bcb94f45bd745290333d2c9bdedc94f36e5").unwrap(),
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
