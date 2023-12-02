use alloy_sol_types::SolCall;
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{
    Actions, NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap,
};
use reth_primitives::{Address, Bytes, U256};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    ADDRESS_TO_TOKENS_2_POOL,
};

pub use uni::UniswapV3Classifier;
pub use sushi::SushiSwapV3Classifier;

macro_rules! V3Swap {
    ($index:ident, $from_address:ident, $target_address:ident, $return_data:ident) => {
        let token_0_delta = $return_data.amount0;
        let token_1_delta = $return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*$target_address.0)
            .copied()
            .unwrap();
        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                U256::from_be_bytes(token_1_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_0_delta.abs().to_be_bytes::<32>()),
                token_1,
                token_0,
            )
        } else {
            (
                U256::from_be_bytes(token_0_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_1_delta.abs().to_be_bytes::<32>()),
                token_0,
                token_1,
            )
        };

        Some(NormalizedSwap {
            $index,
            from: $from_address,
            pool: $target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    };
}

macro_rules! V3Mint {
    ($index:ident, $from_address:ident, $target_address:ident, $calldata:ident, $return_data:ident) => {
        let token_0_delta = $return_data.amount0;
        let token_1_delta = $return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*$target_address.0)
            .copied()
            .unwrap();

        Some(NormalizedMint {
            $index,
            from: $from_address,
            recipient: $call_data.recipient,
            to: $target_address,
            token: vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta],
        })
        
    };
}

macro_rules! V3Burn {
    ($index:ident, $from_address:ident, $target_address:ident, $return_data:ident) => {
        let token_0_delta = $return_data.amount0;
        let token_1_delta = $return_data.amount1;

        let token_0_delta: U256 = $return_data.amount0;
        let token_1_delta: U256 = $return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*$target_address.0)
            .copied()
            .unwrap();

        Some(NormalizedBurn {
            to: $target_address,
            recipient: $target_address,
            $index,
            from: $from_address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        })
        
    };
}

macro_rules! V3Collect {
    ($index:ident, $from_address:ident, $target_address:ident, $calldata:ident, $return_data:ident) => {
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*$target_address.0)
            .copied()
            .unwrap();
        Some(NormalizedCollect {
            $index,
            from: $from_address,
            recipient: $calldata.recipient,
            to: $target_address,
            token: vec![token0, token1],
            amount: vec![U256::from($return_data.amount0), U256::from($return_data.amount1)],
        })
        
    };
}


mod uni {
    use super::*;
    use crate::UniswapV3::{
        burnCall, burnReturn, collectCall, collectReturn, mintCall, mintReturn, swapCall,
        swapReturn, UniswapV3Calls,
    };

    action_impl!(
        V3SwapImpl,
        Swap,
        swapCall,
        UniswapV3,
        return_data: true,
        |index, from_address: Address, target_address: Address, return_data: swapReturn| {
            V3Swap!(index, from_address, target_address, return_data);
        }
    );

    action_impl!(
        V3MintImpl,
        Mint,
        mintCall,
        UniswapV3,
        return_data: true,
        call_data: true,
        |index,
         from_address: Address,
         target_address: Address,
         call_data: mintCall,
         return_data: mintReturn| {
             V3Mint!(index, from_address, target_address, call_data, return_data);
        }
    );

    action_impl!(
        V3BurnImpl,
        Burn,
        burnCall,
        UniswapV3,
        return_data: true,
        |index, from_address: Address, target_address: Address, return_data: burnReturn| {
             V3Burn!(index, from_address, target_address, return_data);
        }
    );

    action_impl!(
        V3CollectImpl,
        Collect,
        collectCall,
        UniswapV3,
        call_data: true,
        return_data: true,
        |
        index,
        from_addr: Address,
        to_addr: Address,
        call_data: collectCall,
        return_data: collectReturn
        | {
            V3Collect!(index, from_addr, to_addr, call_data, return_data);
        }
    );

    action_dispatch!(UniswapV3Classifier, V3SwapImpl, V3BurnImpl, V3MintImpl, V3CollectImpl);
}

mod sushi {
    use super::*;
    use crate::SushiSwapV3::{
        burnCall, burnReturn, collectCall, collectReturn, mintCall, mintReturn, swapCall,
        swapReturn, SushiSwapV3Calls,
    };

    action_impl!(
        V3SwapImpl,
        Swap,
        swapCall,
        SushiSwapV3,
        return_data: true,
        |index, from_address: Address, target_address: Address, return_data: swapReturn| {
            V3Swap!(index, from_address, target_address, return_data);
        }
    );

    action_impl!(
        V3MintImpl,
        Mint,
        mintCall,
        SushiSwapV3,
        return_data: true,
        call_data: true,
        |index,
         from_address: Address,
         target_address: Address,
         call_data: mintCall,
         return_data: mintReturn| {
             V3Mint!(index, from_address, target_address, call_data, return_data);
        }
    );

    action_impl!(
        V3BurnImpl,
        Burn,
        burnCall,
        SushiSwapV3,
        return_data: true,
        |index, from_address: Address, target_address: Address, return_data: burnReturn| {
             V3Burn!(index, from_address, target_address, return_data);
        }
    );

    action_impl!(
        V3CollectImpl,
        Collect,
        collectCall,
        SushiSwapV3,
        call_data: true,
        return_data: true,
        |
        index,
        from_addr: Address,
        to_addr: Address,
        call_data: collectCall,
        return_data: collectReturn
        | {
            V3Collect!(index, from_addr, to_addr, call_data, return_data);
        }
    );

    action_dispatch!(SushiSwapV3Classifier, V3SwapImpl, V3BurnImpl, V3MintImpl, V3CollectImpl);
}



#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use reth_primitives::B256;

    use super::*;
    use crate::*;

    #[test]
    fn test_uni_v3_collect() {
        let classifier = UniswapV3Classifier::default();

        let sig: &[u8] = &UniswapV3::collectCall::SELECTOR;
        println!("SEL: {:?}", Bytes::from(sig));
        let index = 9;
        let calldata = Bytes::from_str("0x4f1eb3d8000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe880000000000000000000000000000000000000000000000000000000000030a0c000000000000000000000000000000000000000000000000000000000003165a0000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap();
        let return_bytes = Bytes::from_str("0x0000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap();
        let from_address = Address::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
        let target_address =
            Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

        let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
        let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

        let logs = vec![
        Log {
            address: Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(),
            topics: vec![
                B256::from_str("0x70935338e69775456a85ddef226c395fb668b63fa0115f5f20610b388e6ca9c0").unwrap().into(),
                B256::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe88").unwrap().into(),
                B256::from_str("0x0000000000000000000000000000000000000000000000000000000000030a0c").unwrap().into(),
                B256::from_str("0x000000000000000000000000000000000000000000000000000000000003165a").unwrap().into(),
            ],
            data: Bytes::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe880000000000000000000000000000000000000000000000000000006f9cc0d1c600000000000000000000000000000000000000000000000de3d53dad63cdb215").unwrap()
            block_hash: Some(B256::from_str("0xc698adc7019160c893fd294bae7f30cd78047fe540575c52911bc7ecbc2ab29f").unwrap()),
            block_number: Some(B256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7F71").unwrap().into()),
            transaction_hash: Some(B256::from_str("0x9d5cb1e9e83fd5b2c95e5d9f68f48cc90cc5f5b0f44c7a61aa0d260c461e9ebe").unwrap()),
            transaction_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000009").unwrap().into()),
            log_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
            removed: false
        },
        ];

        let res = classifier.dispatch(
            sig,
            index,
            data,
            return_bytes,
            from_address,
            target_address,
            &logs,
        );

        assert!(res.is_some());

        let action = res.unwrap();
        assert!(action.is_collect());

        let collect = match action {
            Actions::Collect(s) => s,
            _ => unreachable!(),
        };

        let expected_collect = NormalizedCollect {
            index:     9,
            from:      from_address,
            token:     vec![
                Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            ],
            to:        target_address,
            recipient: Address::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap(),
            amount:    vec![
                B256::from_low_u64_be(479371252166).into(),
                B256::from_str(
                    "0x00000000000000000000000000000000000000000000000DE3D53DAD63CDB215",
                )
                .unwrap()
                .into(),
            ],
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
        let from_address = Address::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
        let target_address =
            Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

        let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
        let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

        let logs = vec![
        Log {
            address: Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(),
            topics: vec![
                B256::from_str("0x0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c").unwrap().into(),
                B256::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe88").unwrap().into(),
                B256::from_str("0x0000000000000000000000000000000000000000000000000000000000030a0c").unwrap().into(),
                B256::from_str("0x000000000000000000000000000000000000000000000000000000000003165a").unwrap().into(),
            ],
            data: Bytes::from_str("0x00000000000000000000000000000000000000000000000001f18297c89002cb0000000000000000000000000000000000000000000000000000006af143783400000000000000000000000000000000000000000000000d45567f936fa135b8").unwrap(), 
            block_hash: Some(B256::from_str("0xc698adc7019160c893fd294bae7f30cd78047fe540575c52911bc7ecbc2ab29f").unwrap()), 
            block_number: Some(B256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7F71").unwrap().into()), 
            transaction_hash: Some(B256::from_str("0x9d5cb1e9e83fd5b2c95e5d9f68f48cc90cc5f5b0f44c7a61aa0d260c461e9ebe").unwrap()), 
            transaction_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000009").unwrap().into()),
            log_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
            removed: false
        },
        ];

        let res = classifier.dispatch(
            sig,
            index,
            data,
            return_bytes,
            from_address,
            target_address,
            &logs,
        );

        assert!(res.is_some());

        let action = res.unwrap();
        assert!(action.is_burn());

        let burn = match action {
            Actions::Burn(s) => s,
            _ => unreachable!(),
        };

        let expected_burn = NormalizedBurn {
            index:     9,
            from:      from_address,
            token:     vec![
                Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            ],
            to:        target_address,
            recipient: target_address,
            amount:    vec![
                B256::from_low_u64_be(459314264116).into(),
                B256::from_str(
                    "0x00000000000000000000000000000000000000000000000d45567f936fa135b8",
                )
                .unwrap()
                .into(),
            ],
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
        let from_address = Address::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap();
        let target_address =
            Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

        let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
        let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

        let logs = vec![
        Log {
            address: Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(), 
            topics: vec![B256::from_str("0x7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde").unwrap().into()], 
            data: Bytes::from_str("0x000000000000000000000000c36442b4a4522e871399cd717abdd847ab11fe8800000000000000000000000000000000000000000000000000082d987468f32000000000000000000000000000000000000000000000000000000000f8c92fd800000000000000000000000000000000000000000000000022b1c8c115e460a4").unwrap(), 
            block_hash: Some(B256::from_str("0x17790eb817da186ab84c36992090a94ba053b83dbb10619fb04cf006b121eb3e").unwrap()), 
            block_number: Some(B256::from_str("0x00000000000000000000000000000000000000000000000000000000011A7E2B").unwrap().into()), 
            transaction_hash: Some(B256::from_str("0xbf46e04f4d44e05064bab5f844aade1fb5d72488c4aaa1fb16103343373daa44").unwrap()), 
            transaction_index: Some(B256::from_str("0x000000000000000000000000000000000000000000000000000000000000005B").unwrap().into()),
            log_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000002").unwrap().into()),
            removed: false
        },
        ];

        let res = classifier.dispatch(
            sig,
            index,
            data,
            return_bytes,
            from_address,
            target_address,
            &logs,
        );

        assert!(res.is_some());

        let action = res.unwrap();
        assert!(action.is_mint());

        let mint = match action {
            Actions::Mint(s) => s,
            _ => unreachable!(),
        };

        let expected_mint = NormalizedMint {
            index:     91,
            from:      from_address,
            token:     vec![
                Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            ],
            to:        target_address,
            recipient: Address::from_str("0xc36442b4a4522e871399cd717abdd847ab11fe88").unwrap(),
            amount:    vec![
                B256::from_low_u64_be(4173934552).into(),
                B256::from_low_u64_be(2499999999788867748).into(),
            ],
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
        let from_address = Address::from_str("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad").unwrap();
        let target_address =
            Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap();

        let bindings = StaticBindings::UniswapV3(UniswapV3_Enum::None);
        let data: StaticReturnBindings = bindings.try_decode(&calldata).unwrap();

        let logs = vec![
        Log {
            address: Address::from_str("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").unwrap(), 
            topics: vec![
                B256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap(),
                B256::from_str("0x0000000000000000000000003fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD").unwrap(),
                B256::from_str("0x0000000000000000000000003fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD").unwrap()
            ],
            data: Bytes::from_str("0x000000000000000000000000000000000000000000000000000000006b2c0f31fffffffffffffffffffffffffffffffffffffffffffffffff2d56e9e92be85db00000000000000000000000000000000000059c03c850f2ae2fa19a8982682ef0000000000000000000000000000000000000000000000018d27a4400c75f3df0000000000000000000000000000000000000000000000000000000000031096").unwrap(), 
            block_hash: Some(B256::from_str("0x7ceb7355e05f351e82525c7b4e04bc6a41673e071bd9ca9ff33a893721e96a63").unwrap()), 
            block_number: Some(B256::from_str("0x00000000000000000000000000000000000000000000000000000000011A8262").unwrap().into()), 
            transaction_hash: Some(B256::from_str("0x681ee84099f113cc13ac4ccc187e702bd64d1f28ef5642e164b405270a012dbd").unwrap()), 
            transaction_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000096").unwrap().into()),
            log_index: Some(B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000003").unwrap().into()),
            removed: false
        }
    ];

        let res = classifier.dispatch(
            sig,
            index,
            data,
            return_bytes,
            from_address,
            target_address,
            &logs,
        );

        assert!(res.is_some());

        let action = res.unwrap();
        assert!(action.is_swap());

        let swap = match action {
            Actions::Swap(s) => s,
            _ => unreachable!(),
        };

        let expected_swap = NormalizedSwap {
            index:      150,
            from:       from_address,
            pool:       target_address,
            token_in:   Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            token_out:  Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            amount_in:  B256::from_low_u64_be(1798049585).into(),
            amount_out: B256::from_low_u64_be(948730519145773605).into(),
        };

        assert_eq!(swap, expected_swap);
    }
}
