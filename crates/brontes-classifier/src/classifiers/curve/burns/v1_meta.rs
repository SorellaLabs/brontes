use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_0Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplremove_liquidity_0CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidity_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.expect("Expected curve_lp_token, found None")];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();



        Ok(NormalizedBurn {
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token: tokens,
            amount: token_amts,
        })

    }
);

// could not find any V1 metapools calling this
action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_1Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplremove_liquidity_1CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidity_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.expect("Expected curve_lp_token, found None")];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();

        Ok(NormalizedBurn {
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_imbalance_0Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplremove_liquidity_imbalance_0CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityImbalance_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.expect("Expected curve_lp_token, found None")];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();

        Ok(NormalizedBurn {
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token: tokens,
            amount: token_amts,
        })

    }
);

// could not find any V1 metapools calling this
action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_imbalance_1Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplremove_liquidity_imbalance_1CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityImbalance_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.expect("Expected curve_lp_token, found None")];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter().enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();

        Ok(NormalizedBurn {
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_one_coin_0Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_0Call,
    log: CurveV1MetapoolImplremove_liquidity_one_coin_0CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityOne_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

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
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token: vec![token_info],
            amount: vec![amt],
        })

    }
);

action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::remove_liquidity_one_coin_1Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_1Call,
    log: CurveV1MetapoolImplremove_liquidity_one_coin_1CallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityOne_field;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

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
            protocol,
            trace_index: info.trace_idx,
            pool: info.from_address,
            from: info.msg_sender,
            recipient: info.msg_sender,
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
    async fn test_curve_v1_metapool_remove_liquidity0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV1MetaPool,
            Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")),
            Some(Address::new(hex!(
                "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            ))),
            Some(Address::new(hex!(
                "dAC17F958D2ee523a2206206994597C13D831ec7"
            ))),
            None,
            Some(Address::new(hex!(
                "6c3F90f043a72FA612cbac8115EE7e52BDe6E490"
            ))),
        );

        let burn = B256::from(hex!(
            "fdf8776b3ba5714db71834acdb08b0741f6760408c29450823def556f28b620c"
        ));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "T".to_string(),
            },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("6c3f90f043a72fa612cbac8115ee7e52bde6e490")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "3Crv".to_string(),
            },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Actions::Burn(NormalizedBurn {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: 1,
            from: Address::new(hex!("95e0022e62A9e13fc9F38A3E288521f2FD042357")),
            recipient: Address::new(hex!("95e0022e62A9e13fc9F38A3E288521f2FD042357")),
            pool: Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            token: vec![token0, token1],
            amount: vec![
                U256::from(125377210391915440945 as u128).to_scaled_rational(18),
                U256::from(2121542034308448729 as u128).to_scaled_rational(18),
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
    async fn test_curve_v1_metapool_remove_liquidity_imbalanced0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV1MetaPool,
            Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")),
            Some(Address::new(hex!(
                "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            ))),
            Some(Address::new(hex!(
                "dAC17F958D2ee523a2206206994597C13D831ec7"
            ))),
            None,
            Some(Address::new(hex!(
                "6c3F90f043a72FA612cbac8115EE7e52BDe6E490"
            ))),
        );

        let burn = B256::from(hex!(
            "f82670e2f08003edaac7da287c105c3989dfc046b0114eb4f3ae7d278da5d581"
        ));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "T".to_string(),
            },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("6c3f90f043a72fa612cbac8115ee7e52bde6e490")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "3Crv".to_string(),
            },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Actions::Burn(NormalizedBurn {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: 1,
            from: Address::new(hex!("a30C1d2f7Bf871FE70827fc438c5A3Fe80eF4f4C")),
            recipient: Address::new(hex!("a30C1d2f7Bf871FE70827fc438c5A3Fe80eF4f4C")),
            pool: Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            token: vec![token0, token1],
            amount: vec![
                U256::from(5782689815360000000000 as u128).to_scaled_rational(18),
                U256::from(60598295710000000000 as u128).to_scaled_rational(18),
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

    // #[brontes_macros::test]
    // async fn test_curve_base_remove_liquidity_one() {
    //     let classifier_utils = ClassifierTestUtils::new().await;
    //     classifier_utils.ensure_protocol(
    //         Protocol::CurveV1MetapoolImpl,
    //         Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
    //         Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
    //         Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
    //         Some(Address::new(hex!(
    //             "fE18be6b3Bd88A2D2A7f928d00292E7a9963CfC6"
    //         ))),
    //         None,
    //         None,
    //         None,
    //     );

    //     let burn = B256::from(hex!(
    //         "054098af5b21c4e95a46b88a2a7d093b83bfdee448a732d3396925f48f4225c3"
    //     ));

    //     let token = TokenInfoWithAddress {
    //         address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
    //         inner: TokenInfo {
    //             decimals: 8,
    //             symbol: "WBTC".to_string(),
    //         },
    //     };

    //     classifier_utils.ensure_token(token.clone());

    //     let eq_action = Actions::Burn(NormalizedBurn {
    //         protocol: Protocol::CurveV1MetapoolImpl,
    //         trace_index: 0,
    //         from: Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
    //         recipient: Address::new(hex!("045929aF66312685d143B96C9d44Ce5ddCBAB768")),
    //         pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
    //         token: vec![token],
    //         amount: vec![U256::from(38855798316741927 as u128).to_scaled_rational(8)],
    //     });

    //     let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
    //         collect_current_node: data
    //             .get_ref(node.data)
    //             .map(|s| s.is_burn())
    //             .unwrap_or_default(),
    //         child_node_to_collect: node
    //             .get_all_sub_actions()
    //             .iter()
    //             .filter_map(|d| data.get_ref(*d))
    //             .any(|action| action.is_burn()),
    //     };

    //     classifier_utils
    //         .contains_action(burn, 0, eq_action, search_fn)
    //         .await
    //         .unwrap();
    // }
}
