use alloy_primitives::{hex, Address};
use brontes_macros::action_impl;
use brontes_types::{normalized_actions::NormalizedSwap, Protocol, ToScaledRational};

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
    log_data: MakerPSMbuyGemCallLogs,
    db_tx: &DB| {

        // For the PSM, the token0 should always be set to DAI and token1 is the gem (USDC or USDP)
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;

        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;

        // The amount of gem token being bought
        let amount_out = call_data.gemAmt.to_scaled_rational(t1_info.decimals);

        // The fee in DAI decimals
        let fee = log_data.BuyGem_field.fee;
        let fee_amount = fee.to_scaled_rational(t0_info.decimals);

        // The amount of DAI being spent, amount out + fee
        let amount_in = &amount_out + &amount_out * fee_amount;


        Some(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.usr,
            pool: info.target_address,
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
    |
    info: CallInfo,
    call_data: sellGemCall,
    log_data: MakerPSMsellGemCallLogs,
    db_tx: &DB| {
        // For the PSM, the token0 is DAI and token1 is the gem (USDC or USDP)
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0).ok()??;
        let t1_info = db_tx.try_fetch_token_info(token_1).ok()??;


        // The amount of gem asset being sold
        let amount_in = call_data.gemAmt.to_scaled_rational(t1_info.decimals);


        // The fee in DAI decimals
        let fee = log_data.SellGem_field.fee;
        let fee_amount = fee.to_scaled_rational(t0_info.decimals);

        // The amount of DAI being received, amount in - fee
        let amount_out = &amount_in - (&amount_in * fee_amount);


        Some(NormalizedSwap {
            protocol: Protocol::MakerPSM,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.usr,
            pool: info.target_address,
            token_in: t0_info,
            token_out: t1_info,
            amount_in,
            amount_out,
        })
    }
);
