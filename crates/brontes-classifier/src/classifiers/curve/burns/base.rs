use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool,
    crate::CurveBase::remove_liquidityCall,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePoolremove_liquidityCallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidity_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();



        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveBasePool,
    crate::CurveBase::remove_liquidity_imbalanceCall,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePoolremove_liquidity_imbalanceCallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityImbalance_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();

        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveBasePool,
    crate::CurveBase::remove_liquidity_one_coinCall,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coinCall,
    log: CurveBasePoolremove_liquidity_one_coinCallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityOne_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token = match call_data.i {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_info = db_tx.try_fetch_token_info(token)?;
        let amt = log.token_amount.to_scaled_rational(token_info.decimals);


        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: vec![token_info],
            amount: vec![amt],
        })

    }
);

#[cfg(test)]
mod tests {

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Actions,
        Node, NodeData, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_base_remote_liquidity() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            Some(Address::new(hex!(
                "fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"
            ))),
            None,
            None,
            None,
        );

        let burn = B256::from(hex!(
            "dbf57244aad3402faa04e1ff19d3af0f89e1ac9aff3dd3830d2d6415b4dfdc0c"
        ));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "renBTC".to_string(),
            },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "WBTC".to_string(),
            },
        };

        let token2 = TokenInfoWithAddress {
            address: Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "sBTC".to_string(),
            },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Actions::Burn(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: 0,
            from: Address::new(hex!("aEBd1F6272Bc7E2d406595cc2E98AAE21a47F03d")),
            recipient: Address::new(hex!("aEBd1F6272Bc7E2d406595cc2E98AAE21a47F03d")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token: vec![token0, token1, token2],
            amount: vec![
                U256::from(135971).to_scaled_rational(8),
                U256::from(27506).to_scaled_rational(8),
                U256::from(2022770990903219 as u128).to_scaled_rational(18),
            ],
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_burn())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_burn()),
        };

        classifier_utils
            .contains_action(burn, 0, eq_action, search_fn)
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_curve_base_remote_liquidity_imbalanced() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            Some(Address::new(hex!(
                "fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"
            ))),
            None,
            None,
            None,
        );

        let burn = B256::from(hex!(
            "3f17151032cb3e3ae039b140e465c3cf3f9ff8cb593109817dd0526eb0300150"
        ));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "renBTC".to_string(),
            },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "WBTC".to_string(),
            },
        };

        let token2 = TokenInfoWithAddress {
            address: Address::new(hex!("fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "sBTC".to_string(),
            },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Actions::Burn(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: 0,
            from: Address::new(hex!("13ca2cf84365BD2daffd4A7e364Ea11388607C37")),
            recipient: Address::new(hex!("13ca2cf84365BD2daffd4A7e364Ea11388607C37")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token: vec![token0, token1, token2],
            amount: vec![
                U256::from(0).to_scaled_rational(8),
                U256::from(50000000).to_scaled_rational(8),
                U256::from(0).to_scaled_rational(18),
            ],
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_burn())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_burn()),
        };

        classifier_utils
            .contains_action(burn, 0, eq_action, search_fn)
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_curve_base_remote_liquidity_one() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveBasePool,
            Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            Some(Address::new(hex!(
                "fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"
            ))),
            None,
            None,
            None,
        );

        let burn = B256::from(hex!(
            "054098af5b21c4e95a46b88a2a7d093b83bfdee448a732d3396925f48f4225c3"
        ));

        let token = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "WBTC".to_string(),
            },
        };

        classifier_utils.ensure_token(token.clone());

        let eq_action = Actions::Burn(NormalizedBurn {
            protocol: Protocol::CurveBasePool,
            trace_index: 0,
            from: Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
            recipient: Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token: vec![token],
            amount: vec![U256::from(38855798316741927 as u128).to_scaled_rational(8)],
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_burn())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_burn()),
        };

        classifier_utils
            .contains_action(burn, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
