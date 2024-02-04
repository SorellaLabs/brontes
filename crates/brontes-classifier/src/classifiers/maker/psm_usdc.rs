use alloy_primitives::Address;
use brontes_macros::action_impl;
use brontes_types::{
    constants::{DAI_ADDRESS, USDC_ADDRESS},
    normalized_actions::NormalizedSwap,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::MakerPSM,
    crate::MakerPSM::buyGemCall,
    Swap,
    [BuyGem],
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: buyGemCall,
    log_data: MakerPSMbuyGemCallLogs,
    db_tx: &DB| {
        let [token_0, token_1] = [DAI_ADDRESS, USDC_ADDRESS];
        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        let fee = log_data.BuyGem_field.fee;

        let amount_usdc = call_data.gemAmt.to_scaled_rational(t1_info.decimals);
        let fee_amount = fee.to_scaled_rational(t0_info.decimals);

        let amount_in = &amount_usdc + &amount_usdc * fee_amount;

        let amount_out = call_data.gemAmt.to_scaled_rational(t1_info.decimals);

        Some(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index,
            from: from_address,
            recipient: call_data.usr,
            pool: target_address,
            token_in: t0_info,
            token_out: t1_info,
            amount_in,
            amount_out,
        })

    }
);

action_impl!(
    Protocol::MakerPSM,
    crate::MakerPSM::sellGemCall,
    Swap,
    [SellGem],
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: sellGemCall,
    log_data: MakerPSMsellGemCallLogs,
    db_tx: &DB| {
        let [token_0, token_1] = [USDC_ADDRESS, DAI_ADDRESS];
        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        let fee = log_data.SellGem_field.fee;

        let amount_usdc = call_data.gemAmt.to_scaled_rational(t0_info.decimals);
        let fee_amount = fee.to_scaled_rational(t1_info.decimals);

        let amount_out = &amount_usdc + &amount_usdc * fee_amount;

        let amount_in = call_data.gemAmt.to_scaled_rational(t0_info.decimals);

        Some(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index,
            from: from_address,
            recipient: call_data.usr,
            pool: target_address,
            token_in: t0_info,
            token_out: t1_info,
            amount_in,
            amount_out,
        })
    }
);
