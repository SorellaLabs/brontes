use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_0Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplRemove_liquidity_0CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.ok_or(eyre::eyre!("Expected 'curve_lp_token', found 'None'"))?];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter()
.enumerate().map(|(i, t)|
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

// could not find any V2 metapools calling this
action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_1Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplRemove_liquidity_1CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.ok_or(eyre::eyre!("Expected 'curve_lp_token', found 'None'"))?];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter()
.enumerate().map(|(i, t)|
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

// could not find any V2 metapools calling this
action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_imbalance_0Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplRemove_liquidity_imbalance_0CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_imbalance_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.ok_or(eyre::eyre!("Expected 'curve_lp_token', found 'None'"))?];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter()
.enumerate().map(|(i, t)|
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

// could not find any V2 metapools calling this
action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_imbalance_1Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplRemove_liquidity_imbalance_1CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_imbalance_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let token_addrs = vec![details.token0, details.curve_lp_token.ok_or(eyre::eyre!("Expected 'curve_lp_token', found 'None'"))?];
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = token_addrs.into_iter()
.enumerate().map(|(i, t)|
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
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_one_coin_0Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_0Call,
    log: CurveV2MetapoolImplRemove_liquidity_one_coin_0CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_one_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let token = match call_data.i {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for burn token, found None"))?,
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

// could not find any V2 metapools calling this
action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::remove_liquidity_one_coin_1Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_1Call,
    log: CurveV2MetapoolImplRemove_liquidity_one_coin_1CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_one_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let token = match call_data.i {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for burn token, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for burn token, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for burn token, found None"))?,
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
        normalized_actions::Action,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_remove_liquidity0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2MetaPool,
            Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            Some(Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F"))),
            Some(Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"))),
            Some(Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"))),
            None,
            Some(Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490"))),
        );

        let burn =
            B256::from(hex!("65205c76ff56bcc1e3b26dc56f02d5990b84aac3baa6469d1d09b8f24581611a"));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            inner:   TokenInfo { decimals: 18, symbol: "STBT".to_string() },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
            inner:   TokenInfo { decimals: 18, symbol: "3Crv".to_string() },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveV2MetaPool,
            trace_index: 1,
            from:        Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            recipient:   Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            pool:        Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token:       vec![token0, token1],
            amount:      vec![
                U256::from(627992358239302043763875_u128).to_scaled_rational(18),
                U256::from(579890756974932941933194_u128).to_scaled_rational(18),
            ],
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_remove_liquidity_one0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2MetaPool,
            Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            Some(Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F"))),
            Some(Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"))),
            Some(Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"))),
            None,
            Some(Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490"))),
        );

        let burn =
            B256::from(hex!("5dba3dc01218fcd657f1a24e4235e901994aeae30d47feff4c2a86f5d0a1f0bd"));

        let token = TokenInfoWithAddress {
            address: Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
            inner:   TokenInfo { decimals: 18, symbol: "3Crv".to_string() },
        };

        classifier_utils.ensure_token(token.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveV2MetaPool,
            trace_index: 1,
            from:        Address::new(hex!("3F7734E28ed8856B89e13137bd2E9112C40ebD51")),
            recipient:   Address::new(hex!("3F7734E28ed8856B89e13137bd2E9112C40ebD51")),
            pool:        Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token:       vec![token],
            amount:      vec![U256::from(183708410783845567136_u128).to_scaled_rational(18)],
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }
}
