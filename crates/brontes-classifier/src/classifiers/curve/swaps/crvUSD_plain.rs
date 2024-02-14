use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurvecrvUSDPlainPoolImpl,
    crate::CurvecrvUSDPlainImpl::exchange_0Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurvecrvUSDPlainPoolImplexchange_0CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: details.protocol,
            pool: info.from_address,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::CurvecrvUSDPlainPoolImpl,
    crate::CurvecrvUSDPlainImpl::exchange_1Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurvecrvUSDPlainPoolImplexchange_1CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: details.protocol,
            pool: info.from_address,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient: info.msg_sender,
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
        Node, NodeData, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_crv_usd_plain_pool_exchange0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurvecrvUSDPlainPool,
            Address::new(hex!("1539c2461d7432cc114b0903f1824079bfca2c92")),
            Address::new(hex!("f939E0A03FB07F59A73314E73794Be0E57ac1b4E")),
            Address::new(hex!("83F20F44975D03b1b09e64809B757c47f942BEeA")),
            None,
            None,
            None,
            None,
        );

        let swap = B256::from(hex!(
            "6ee69de7d6718a43359e1c7f579dd4ec6958a1079a7343a978391884c2306ede"
        ));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("f939E0A03FB07F59A73314E73794Be0E57ac1b4E")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "crvUSD".to_string(),
            },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("83F20F44975D03b1b09e64809B757c47f942BEeA")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "sDAI".to_string(),
            },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::CurvecrvUSDPlainPool,
            trace_index: 1,
            from: Address::new(hex!("71C91A8950f6a3025EdC754b2f44291E011AA45C")),
            recipient: Address::new(hex!("71C91A8950f6a3025EdC754b2f44291E011AA45C")),
            pool: Address::new(hex!("1539c2461d7432cc114b0903f1824079bfca2c92")),
            token_in,
            amount_in: U256::from_str("1949941364975630672628")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("858368064339421192")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_swap())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_swap()),
        };

        classifier_utils
            .contains_action(swap, 0, eq_action, search_fn)
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_curve_crv_usd_plain_pool_exchange1() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2PlainPool,
            Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7")),
            None,
            None,
            None,
            None,
        );

        let swap = B256::from(hex!(
            "088ca9fd8ea73ecd33ba1bef7aafd1bd57a22275d15d6a79c7f3889d88ba3720"
        ));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "cvxCRV".to_string(),
            },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "CRV".to_string(),
            },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2PlainPool,
            trace_index: 1,
            from: Address::new(hex!("554EF7d3C2E629ab3DD4F3d22717741F22d3B2d7")),
            recipient: Address::new(hex!("554EF7d3C2E629ab3DD4F3d22717741F22d3B2d7")),
            pool: Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            token_in,
            amount_in: U256::from_str("5738343996295056106530")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("5632479022165211497524")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_swap())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_swap()),
        };

        classifier_utils
            .contains_action(swap, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
