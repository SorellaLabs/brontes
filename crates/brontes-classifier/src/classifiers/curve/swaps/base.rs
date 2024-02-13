use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    constants::{ETH_ADDRESS, WETH_ADDRESS},
    normalized_actions::NormalizedSwap,
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool,
    crate::CurveBase::exchangeCall,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePoolexchangeCallLogs,
    db_tx: &DB
    |{
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token in, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token in, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::CurveBasePool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })

    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Actions,
        Node, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_v1_base_exchange() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            None,
            None,
            None,
            None,
        );

        let swap =
            B256::from(hex!("6987133dd8ee7f5f76615a7484418905933625305a948350b38e924a905c0ef6"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner:   TokenInfo { decimals: 8, symbol: "WBTC".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner:   TokenInfo { decimals: 8, symbol: "renBTC".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::CurveBasePool,
            trace_index: 0,
            from: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            recipient: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token_in,
            amount_in: U256::from_str("61733447").unwrap().to_scaled_rational(8),
            token_out,
            amount_out: U256::from_str("61329579").unwrap().to_scaled_rational(8),
            msg_value: U256::ZERO,
        });

        let search_fn = |node: &Node<Actions>| TreeSearchArgs {
            collect_current_node:  node.data.is_swap(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .any(|action| action.is_swap()),
        };

        classifier_utils
            .contains_action(swap, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
