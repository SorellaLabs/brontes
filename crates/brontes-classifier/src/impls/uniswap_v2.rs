use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    UniswapV2::{burnCall, mintCall, swapCall, Burn, Mint, Swap, UniswapV2Calls},
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    None,
    true,
    false,
    |index, from_address: H160, target_address: H160, data: Option<Swap>| {
        let data = data?;
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: data.amount0In,
                amount_out: data.amount1Out,
            })
        }
    }
);

action_impl!(
    V2MintImpl,
    Mint,
    mintCall,
    Some(UniswapV2),
    true,
    false,
    |index,
     from_address: H160,
     target_address: H160,
     call_data: mintCall,
     log_data: Option<Mint>| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        Some(NormalizedMint {
            recipient: H160(call_data.to.0 .0),
            from: from_address,
            index,
            to: target_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);

action_impl!(
    V2BurnImpl,
    Burn,
    burnCall,
    Some(UniswapV2),
    true,
    false,
    |index,
     from_address: H160,
     target_address: H160,
     call_data: burnCall,
     log_data: Option<Burn>| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        Some(NormalizedBurn {
            recipient: H160(call_data.to.0 .0),
            to: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);

action_dispatch!(UniswapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
action_dispatch!(SushiSwapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);


#[cfg(test)]
mod tests {

    use std::str::FromStr;
    use reth_primitives::H256;
    use super::*;
    use crate::{*, UniswapV2, UniswapV3};

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
    
}