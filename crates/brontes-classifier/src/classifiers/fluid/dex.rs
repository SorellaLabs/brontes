use brontes_macros::{action_impl, discovery_impl};
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedNewPool, NormalizedSwap},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};
use alloy_primitives::Address;
use alloy_sol_types::SolType;
use alloy_dyn_abi::{DynSolType, DynSolValue};

action_impl!(
    Protocol::FluidDEX,
    crate::FluidDexFactory::deployDexCall,
    NewPool,
    [DexT1Deployed],
    logs:true,
    |info: CallInfo, log_data:FluidDEXDeployDexCallLogs,_| {
        let dex_t1_deployed=log_data.dex_t1_deployed_field?;
        let pool_address=dex_t1_deployed.dex;
        let tokens=vec![dex_t1_deployed.supplyToken, dex_t1_deployed.borrowToken];

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::FluidDEX,
            pool_address,
            tokens,
        })
    }
);

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


