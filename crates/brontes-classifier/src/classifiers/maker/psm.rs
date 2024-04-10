use alloy_primitives::{hex, Address};
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, Protocol, ToScaledRational,
};

pub const USDC_PSM_ADDRESS: Address = Address::new(hex!(
    "89B78CfA322F6C5dE0aBcEecab66Aee45393cC5A
"
));
pub const USDP_PSM_ADDRESS: Address = Address::new(hex!(
    "961Ae24a1Ceba861D1FDf723794f6024Dc5485Cf
    "
));

action_impl!(
    Protocol::MakerPSM,
    crate::MakerPSM::buyGemCall,
    Swap,
    [BuyGem],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: buyGemCall,
    log_data: MakerPSMBuyGemCallLogs,
    db_tx: &DB| {

        // For the PSM, the token0 should always be set to DAI and token1 is the gem (USDC or USDP)
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        // The amount of gem token being bought
        let amount_out = call_data.gemAmt.to_scaled_rational(t1_info.decimals);

        // The fee in DAI decimals
        let fee = log_data.buy_gem_field?.fee;
        let fee_amount = fee.to_scaled_rational(t0_info.decimals);

        // The amount of DAI being spent, amount out + fee
        let amount_in = &amount_out + &amount_out * fee_amount;


        Ok(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.usr,
            pool: info.target_address,
            token_in: t0_info,
            token_out: t1_info,
            amount_in,
            amount_out,
            msg_value: info.msg_value,
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
    |
    info: CallInfo,
    call_data: sellGemCall,
    log_data: MakerPSMSellGemCallLogs,
    db_tx: &DB| {
        // For the PSM, the token0 is DAI and token1 is the gem (USDC or USDP)
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;


        // The amount of gem asset being sold
        let amount_in = call_data.gemAmt.to_scaled_rational(t1_info.decimals);


        // The fee in DAI decimals
        let fee = log_data.sell_gem_field?.fee;
        let fee_amount = fee.to_scaled_rational(t0_info.decimals);

        // The amount of DAI being received, amount in - fee
        let amount_out = &amount_in - (&amount_in * fee_amount);


        Ok(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.usr,
            pool: info.target_address,
            token_in: t0_info,
            token_out: t1_info,
            amount_in,
            amount_out,
            msg_value: info.msg_value,
        })
    }
);
