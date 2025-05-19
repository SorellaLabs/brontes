use brontes_macros::{action_impl, discovery_impl};
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};
use alloy_primitives::Address;
use alloy_sol_types::SolType;
use alloy_dyn_abi::{DynSolType, DynSolValue};

action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::swapInCall,
    Swap,
    [Swap],
    call_data:true,
    return_data:true,
    |
    info: CallInfo,
    call_data:swapInCall,
    return_data:swapInReturn,
    db_tx: &DB| {
        let recipient=call_data.to_;
        let swap_0_to_1=call_data.swap0to1_;
        let amount_in=call_data.amountIn_;
        let amount_out=return_data.amountOut_;
        let details=db_tx.get_protocol_details(info.target_address)?;
        let (token_in, token_out)=if swap_0_to_1{
            (details.token0, details.token1)
        }else{
            (details.token1, details.token0)
        };

        let token_in=db_tx.try_fetch_token_info(token_in)?;
        let token_out=db_tx.try_fetch_token_info(token_out)?;

        let amount_in= amount_in.to_scaled_rational(token_in.decimals);
        let amount_out= amount_out.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            pool: info.target_address,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::swapInWithCallbackCall,
    Swap,
    [Swap],
    call_data:true,
    return_data :true,
    |
    info: CallInfo,
    call_data:swapInWithCallbackCall,
    return_data:swapInWithCallbackReturn,
    db_tx: &DB| {
        let recipient=call_data.to_;
        let swap_0_to_1=call_data.swap0to1_;
        let amount_in=call_data.amountIn_;
        let amount_out=return_data.amountOut_;
        let details=db_tx.get_protocol_details(info.target_address)?;
        let (token_in, token_out)=if swap_0_to_1{
            (details.token0, details.token1)
        }else{
            (details.token1, details.token0)
        };

        let token_in=db_tx.try_fetch_token_info(token_in)?;
        let token_out=db_tx.try_fetch_token_info(token_out)?;

        let amount_in= amount_in.to_scaled_rational(token_in.decimals);
        let amount_out= amount_out.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            pool: info.target_address,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::swapOutCall,
    Swap,
    [..],
    call_data:true,
    return_data :true,
    |
    info: CallInfo,
    call_data:swapOutCall,
    return_data:swapOutReturn,
    db_tx: &DB| {
        let recipient=call_data.to_;
        let swap_0_to_1=call_data.swap0to1_;
        let amount_in=return_data.amountIn_;
        let amount_out=call_data.amountOut_;
        let details=db_tx.get_protocol_details(info.target_address)?;
        let (token_in, token_out)=if swap_0_to_1{
            (details.token0, details.token1)
        }else{
            (details.token1, details.token0)
        };

        let token_in=db_tx.try_fetch_token_info(token_in)?;
        let token_out=db_tx.try_fetch_token_info(token_out)?;

        let amount_in= amount_in.to_scaled_rational(token_in.decimals);
        let amount_out= amount_out.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            pool: info.target_address,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::swapOutWithCallbackCall,
    Swap,
    [..],
    call_data:true,
    return_data :true,
    |
    info: CallInfo,
    call_data:swapOutWithCallbackCall,
    return_data:swapOutWithCallbackReturn,
    db_tx: &DB| {
        let recipient=call_data.to_;
        let swap_0_to_1=call_data.swap0to1_;
        let amount_in=return_data.amountIn_;
        let amount_out=call_data.amountOut_;
        let details=db_tx.get_protocol_details(info.target_address)?;
        let (token_in, token_out)=if swap_0_to_1{
            (details.token0, details.token1)
        }else{
            (details.token1, details.token0)
        };

        let token_in=db_tx.try_fetch_token_info(token_in)?;
        let token_out=db_tx.try_fetch_token_info(token_out)?;

        let amount_in= amount_in.to_scaled_rational(token_in.decimals);
        let amount_out= amount_out.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            pool: info.target_address,
            msg_value: info.msg_value,
        })
    }
);



action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::depositPerfectCall,
    Mint,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: depositPerfectCall, return_data: depositPerfectReturn, db: &DB| {
        let recipient=info.msg_sender;
        let pool=info.target_address;

        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let token_0_amount=return_data.token0Amt_.to_scaled_rational(tokens[0].decimals);
        let token_1_amount=return_data.token1Amt_.to_scaled_rational(tokens[1].decimals);

        Ok(NormalizedMint {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            pool,
            token:tokens,
            amount: vec![token_0_amount, token_1_amount]
        })
    }
);



action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::depositCall,
    Mint,
    [..],
    call_data: true,
    |info: CallInfo, call_data: depositCall, db: &DB| {
        let recipient=info.msg_sender;
        let pool=info.target_address;

        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let token_0_amount=call_data.token0Amt_.to_scaled_rational(tokens[0].decimals);
        let token_1_amount=call_data.token1Amt_.to_scaled_rational(tokens[1].decimals);

        Ok(NormalizedMint {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            pool,
            token:tokens,
            amount: vec![token_0_amount, token_1_amount]
        })
    }
);


action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::withdrawPerfectCall,
    Burn,
    [..],
    call_data: true,
    return_data: true,
    |info: CallInfo, call_data: withdrawPerfectCall,return_data: withdrawPerfectReturn, db: &DB| {
        let recipient=info.msg_sender;
        let pool=info.target_address;
        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let token_0_amount=return_data.token0Amt_.to_scaled_rational(tokens[0].decimals);
        let token_1_amount=return_data.token1Amt_.to_scaled_rational(tokens[1].decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            pool,
            token: tokens,
            amount: vec![token_0_amount, token_1_amount]
        })
    }
);




action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexT1::withdrawCall,
    Burn,
    [..],
    call_data: true,
    |info: CallInfo, call_data: withdrawCall, db: &DB| {
        let recipient=call_data.to_;
        let pool=info.target_address;
        let details=db.get_protocol_details(pool)?;
        let tokens=details.get_tokens();
        let tokens=tokens.iter().map(|token| db.try_fetch_token_info(*token)).collect::<Result<Vec<_>, _>>()?;

        let token_0_amount=call_data.token0Amt_.to_scaled_rational(tokens[0].decimals);
        let token_1_amount=call_data.token1Amt_.to_scaled_rational(tokens[1].decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::FluidDEX,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient,
            pool,
            token: tokens,
            amount: vec![token_0_amount, token_1_amount]
        })
    }
);




discovery_impl!(
    FluidDexFactoryDiscovery,
    crate::FluidDexFactory::deployDexCall,
    0x46978CD477A496028A18c02F07ab7F35EDBa5A54,
    |deployed_address: Address, trace_index: u64, call_data: deployDexCall ,_| async move {
        let pool_t1_creation_code = &call_data.dexDeploymentData_[4..];

        let dex_t1_creation_code_type = DynSolType::Tuple(vec![DynSolType::Address, DynSolType::Address, DynSolType::Uint(256)]);

        let decoded_data = dex_t1_creation_code_type.abi_decode(pool_t1_creation_code).expect("Failed to decode pool T1 creation code");
        let DynSolValue::Tuple(values) = decoded_data else { panic!("Expected tuple") };
        let [DynSolValue::Address(token0), DynSolValue::Address(token1), _] = values.as_slice() else { panic!("Invalid tuple structure") };

        let tokens = vec![*token0, *token1];

        vec![NormalizedNewPool {
            trace_index,
            protocol: Protocol::FluidDEX,
            pool_address: deployed_address,
            tokens,
        }]
    }
);
