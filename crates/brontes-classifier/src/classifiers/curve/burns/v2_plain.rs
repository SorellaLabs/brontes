use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_0Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplRemove_liquidity_0CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
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

// could not find any V2 Plain Pools calling this
action_impl!(
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_1Call,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplRemove_liquidity_1CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
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
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_imbalance_0Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplRemove_liquidity_imbalance_0CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_imbalance_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
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
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_imbalance_1Call,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplRemove_liquidity_imbalance_1CallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_imbalance_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;
        let protocol = details.protocol;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
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
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_one_coin_0Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_0Call,
    log: CurveV2PlainPoolImplRemove_liquidity_one_coin_0CallLogs,
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

// could not find any V2 metapools calling this
action_impl!(
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::remove_liquidity_one_coin_1Call,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coin_1Call,
    log: CurveV2PlainPoolImplRemove_liquidity_one_coin_1CallLogs,
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
    async fn test_curve_v2_plain_pool_remove_liquidity0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2PlainPool,
            Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            Some(Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7"))),
            None,
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("01263e507a992709a0a53c014ead38c109edc7aa08bc8329a94d051530da8258"));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            inner:   TokenInfo { decimals: 18, symbol: "CRV".to_string() },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7")),
            inner:   TokenInfo { decimals: 18, symbol: "cvxCRV".to_string() },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveV2PlainPool,
            trace_index: 1,
            from:        Address::new(hex!("598C5E19a132a5c433a80C908f05D87bFDaAC4ae")),
            recipient:   Address::new(hex!("598C5E19a132a5c433a80C908f05D87bFDaAC4ae")),
            pool:        Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            token:       vec![token0, token1],
            amount:      vec![
                U256::from(7558238951551444616838_u128).to_scaled_rational(18),
                U256::from(33415347097773187822792_u128).to_scaled_rational(18),
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
    async fn test_curve_v2_plain_pool_remove_liquidity_imbalanced0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2PlainPool,
            Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            Some(Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7"))),
            None,
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("d6ea9a7ee442796536490731fb1236fad076ce6994aeda37735670f3690ad06a"));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            inner:   TokenInfo { decimals: 18, symbol: "CRV".to_string() },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7")),
            inner:   TokenInfo { decimals: 18, symbol: "cvxCRV".to_string() },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveV2PlainPool,
            trace_index: 1,
            from:        Address::new(hex!("a0f75491720835b36edC92D06DDc468D201e9b73")),
            recipient:   Address::new(hex!("a0f75491720835b36edC92D06DDc468D201e9b73")),
            pool:        Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            token:       vec![token0, token1],
            amount:      vec![
                U256::from(827904920210000000000000_u128).to_scaled_rational(18),
                U256::from(332024620000000000000000_u128).to_scaled_rational(18),
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
    async fn test_curve_v2_plain_pool_remove_liquidity_one0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2PlainPool,
            Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            Address::new(hex!("D533a949740bb3306d119CC777fa900bA034cd52")),
            Some(Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7"))),
            None,
            None,
            None,
            None,
        );

        let burn =
            B256::from(hex!("c6efe6fdf9d1e872765c98730455a9803781958fab7774c48a0be9148ad1165e"));

        let token = TokenInfoWithAddress {
            address: Address::new(hex!("62B9c7356A2Dc64a1969e19C23e4f579F9810Aa7")),
            inner:   TokenInfo { decimals: 18, symbol: "cvxCRV".to_string() },
        };

        classifier_utils.ensure_token(token.clone());

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::CurveV2PlainPool,
            trace_index: 1,
            from:        Address::new(hex!("F94F7b6b956225BcE60A5f0C7B82D347071E48dC")),
            recipient:   Address::new(hex!("F94F7b6b956225BcE60A5f0C7B82D347071E48dC")),
            pool:        Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            token:       vec![token],
            amount:      vec![U256::from(915720089431618525538_u128).to_scaled_rational(18)],
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
