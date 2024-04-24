use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedNewPool, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::Dodo,
    crate::DodoDPPFactory::initDODOPrivatePoolCall,
    NewPool,
    [NewDPP],
    logs: true,
    |info: CallInfo, log_data: DodoInitDODOPrivatePoolCallLogs, _| {
        let logs = log_data.new_d_p_p_field?;

        let base_token = logs.baseToken;
        let quote_token = logs.quoteToken;

        Ok(NormalizedNewPool {
            trace_index: info.trace_idx,
            protocol: Protocol::Dodo,
            pool_address: logs.dpp,
            tokens: vec![base_token, quote_token],
        })
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDPPPool::sellBaseCall,
    Swap,
    [DODOSwap],
    logs: true,
    |info: CallInfo, log_data: DodoSellBaseCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_swap_field?;

        let token_in = db.try_fetch_token_info(logs.fromToken)?;
        let token_out = db.try_fetch_token_info(logs.toToken)?;

        let amount_in = logs.fromAmount.to_scaled_rational(token_in.decimals);
        let amount_out = logs.toAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.trader,
            recipient: logs.receiver,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDPPPool::sellQuoteCall,
    Swap,
    [DODOSwap],
    logs: true,
    |info: CallInfo, log_data: DodoSellQuoteCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_swap_field?;

        let token_in = db.try_fetch_token_info(logs.fromToken)?;
        let token_out = db.try_fetch_token_info(logs.toToken)?;

        let amount_in = logs.fromAmount.to_scaled_rational(token_in.decimals);
        let amount_out = logs.toAmount.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.trader,
            recipient: logs.receiver,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::Dodo,
    crate::DodoDPPPool::flashLoanCall,
    FlashLoan,
    [DODOFlashLoan],
    logs: true,
    |info: CallInfo, log_data: DodoFlashLoanCallLogs, db: &DB| {
        let logs = log_data.d_o_d_o_flash_loan_field?;

        let details = db.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let token_one = db.try_fetch_token_info(token_0)?;
        let token_two = db.try_fetch_token_info(token_1)?;

        let amount_one = logs.baseAmount.to_scaled_rational(token_one.decimals);
        let amount_two = logs.quoteAmount.to_scaled_rational(token_two.decimals);

        Ok(NormalizedFlashLoan {
            protocol: Protocol::Dodo,
            trace_index: info.trace_idx,
            from: logs.borrower,
            pool: info.target_address,
            receiver_contract: logs.assetTo,
            assets: vec![token_one, token_two],
            amounts: vec![amount_one, amount_two],

            // empty
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
            msg_value: info.msg_value
        })
    }
);
