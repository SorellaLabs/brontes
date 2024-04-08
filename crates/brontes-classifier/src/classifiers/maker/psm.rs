use alloy_primitives::{hex, Address};
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedFlashLoan, NormalizedSwap}, structured_trace::CallInfo, Protocol, ToScaledRational,
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

action_impl!(
    Protocol::MakerDssFlash,
    crate::MakerDssFlash::flashLoanCall,
    FlashLoan,
    [FlashLoan],
    call_data: true,
    logs: true,
    |call_info: CallInfo, call_data: flashLoanCall, log_data: MakerDssFlashFlashLoanCallLogs, db_tx: &DB| {
        let token = db_tx.try_fetch_token_info(call_data.token)?;
        let amount = call_data.amount.to_scaled_rational(token.decimals);

        Ok(NormalizedFlashLoan {
            protocol: Protocol::MakerDssFlash,
            trace_index: call_info.trace_idx,
            from: call_info.from_address,
            pool: call_info.target_address,
            msg_value: call_info.msg_value,
            receiver_contract: call_data.receiver,
            assets: vec![token],
            amounts: vec![amount],

            // Empty 
            aave_mode: None,
            child_actions: vec![],
            repayments: vec![],
            fees_paid: vec![],
        })
    }
);


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        constants::WETH_ADDRESS, db::token_info::{TokenInfo, TokenInfoWithAddress}, normalized_actions::Actions,
        Protocol::BalancerV2, TreeSearchBuilder,
    };
    use reth_primitives::U256;

    use super::*;

    #[brontes_macros::test]
    async fn test_maker_dss_flashloan() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let flashloan_tx =
            B256::from(hex!("8e2d6af376182807f0671f1504767c7723c49921344ce4f5799d8ba2d30d014c"));

        let eq_action = Actions::FlashLoan(NormalizedFlashLoan {
            protocol:          Protocol::BalancerV2,
            trace_index:       2,
            from:              Address::new(hex!("97c1a26482099363cb055f0f3ca1d6057fe55447")),
            pool:              Address::new(hex!("ba12222222228d8ba445958a75a0704d566bf2c8")),
            receiver_contract: Address::new(hex!("97c1a26482099363cb055f0f3ca1d6057fe55447")),
            assets:            vec![TokenInfoWithAddress {
                address: Address::new(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
                inner:   TokenInfo { decimals: 18, symbol: "WETH".to_string() },
            }],
            amounts:           vec![U256::from_str("653220647374307183")
                .unwrap()
                .to_scaled_rational(18)],
            aave_mode:         None,
            child_actions:     vec![],
            repayments:        vec![],
            fees_paid:         vec![],
            msg_value:         U256::ZERO,
        });

        classifier_utils
            .contains_action_except(
                flashloan_tx,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_flash_loan),
                &["child_actions"],
            )
            .await
            .unwrap(); 

    }
}