use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::ClipperExchange,
    crate::ClipperExchange::swapCall,
    Swap,
    [..Swapped],
    logs: true,
    |
    info: CallInfo,
    logs: ClipperExchangeSwapCallLogs,
    db_tx: &DB| {
            let logs = logs.swapped_field?;
            let recipient = logs.recipient;
            let token_in = db_tx.try_fetch_token_info(logs.inAsset)?;
            let token_out = db_tx.try_fetch_token_info(logs.outAsset)?;
            let amount_in = logs.inAmount.to_scaled_rational(token_in.decimals);
            let amount_out = logs.outAmount.to_scaled_rational(token_out.decimals);
            Ok(NormalizedSwap {
                protocol: Protocol::ClipperExchange,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                pool: info.target_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: info.msg_value,
            })
        }
);

action_impl!(
    Protocol::ClipperExchange,
    crate::ClipperExchange::sellEthForTokenCall,
    Swap,
    [..Swapped],
    logs: true,
    |
    info: CallInfo,
    logs: ClipperExchangeSellEthForTokenCallLogs,
    db_tx: &DB| {
            let logs = logs.swapped_field?;
            let recipient = logs.recipient;
            let token_in = db_tx.try_fetch_token_info(logs.inAsset)?;
            let token_out = db_tx.try_fetch_token_info(logs.outAsset)?;
            let amount_in = logs.inAmount.to_scaled_rational(token_in.decimals);
            let amount_out = logs.outAmount.to_scaled_rational(token_out.decimals);
            Ok(NormalizedSwap {
                protocol: Protocol::ClipperExchange,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                pool: info.target_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: info.msg_value,
            })
        }
);

action_impl!(
    Protocol::ClipperExchange,
    crate::ClipperExchange::sellTokenForEthCall,
    Swap,
    [..Swapped],
    logs: true,
    |
    info: CallInfo,
    logs: ClipperExchangeSellTokenForEthCallLogs,
    db_tx: &DB| {
            let logs = logs.swapped_field?;
            let recipient = logs.recipient;
            let token_in = db_tx.try_fetch_token_info(logs.inAsset)?;
            let token_out = db_tx.try_fetch_token_info(logs.outAsset)?;
            let amount_in = logs.inAmount.to_scaled_rational(token_in.decimals);
            let amount_out = logs.outAmount.to_scaled_rational(token_out.decimals);
            Ok(NormalizedSwap {
                protocol: Protocol::ClipperExchange,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                pool: info.target_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: info.msg_value,
            })
        }
);

action_impl!(
    Protocol::ClipperExchange,
    crate::ClipperExchange::transmitAndSwapCall,
    Swap,
    [..Swapped],
    logs: true,
    |
    info: CallInfo,
    logs: ClipperExchangeTransmitAndSwapCallLogs,
    db_tx: &DB| {
            let logs = logs.swapped_field?;
            let recipient = logs.recipient;
            let token_in = db_tx.try_fetch_token_info(logs.inAsset)?;
            let token_out = db_tx.try_fetch_token_info(logs.outAsset)?;
            let amount_in = logs.inAmount.to_scaled_rational(token_in.decimals);
            let amount_out = logs.outAmount.to_scaled_rational(token_out.decimals);
            Ok(NormalizedSwap {
                protocol: Protocol::ClipperExchange,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                pool: info.target_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: info.msg_value,
            })
        }
);

action_impl!(
    Protocol::ClipperExchange,
    crate::ClipperExchange::transmitAndSellTokenForEthCall,
    Swap,
    [..Swapped],
    logs: true,
    |
    info: CallInfo,
    logs: ClipperExchangeTransmitAndSellTokenForEthCallLogs,
    db_tx: &DB| {
            let logs = logs.swapped_field?;
            let recipient = logs.recipient;
            let token_in = db_tx.try_fetch_token_info(logs.inAsset)?;
            let token_out = db_tx.try_fetch_token_info(logs.outAsset)?;
            let amount_in = logs.inAmount.to_scaled_rational(token_in.decimals);
            let amount_out = logs.outAmount.to_scaled_rational(token_out.decimals);
            Ok(NormalizedSwap {
                protocol: Protocol::ClipperExchange,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient,
                pool: info.target_address,
                token_in,
                token_out,
                amount_in,
                amount_out,
                msg_value: info.msg_value,
            })
        }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action,
        Protocol::ClipperExchange, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_clipper_exchange_transmit_and_sell_token_for_eth() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("3d9186d1cce43df1b3365d2faa19a35093412c583a9130e12e81cb8d389c3e45"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    ClipperExchange,
            trace_index: 0,
            from:        Address::new(hex!("aeaC71B09AeaeDC6A52CEe06373a648CAd620c20")),
            recipient:   Address::new(hex!("aeaC71B09AeaeDC6A52CEe06373a648CAd620c20")),
            pool:        Address::new(hex!("655eDCE464CC797526600a462A8154650EEe4B77")),
            token_in:    TokenInfoWithAddress::usdc(),
            amount_in:   U256::from_str("1213920000").unwrap().to_scaled_rational(6),
            token_out:   TokenInfoWithAddress::weth(),
            amount_out:  U256::from_str("360342259234585088")
                .unwrap()
                .to_scaled_rational(18),
            msg_value:   U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }
}
