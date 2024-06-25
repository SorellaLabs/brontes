use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToFloatNearest, ToScaledRational
};

action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_0Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplExchange_0CallLogs,
    db_tx: &DB|{
        let log = log.token_exchange_field?;

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

        println!("CurveV2Meta swap");
        println!("token_in: {token_in:?}");
        println!("token_out: {token_out:?}");
        println!("amount_in: {}", amount_in.clone().to_float());
        println!("amount_out: {}", amount_out.clone().to_float());
        println!("info: {info:?}");

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
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_1Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplExchange_1CallLogs,
    db_tx: &DB|{
        let log = log.token_exchange_field?;

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
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_underlying_0Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplExchange_underlying_0CallLogs,
    db_tx: &DB|{
        let log = log.token_exchange_underlying_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;

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
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_underlying_1Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplExchange_underlying_1CallLogs,
    db_tx: &DB|{
        let log = log.token_exchange_underlying_field?;

        let details = db_tx.get_protocol_details(info.from_address)?;

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
        normalized_actions::Action,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_exchange0() {
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

        let swap =
            B256::from(hex!("c32dc9024f2680772ce9d6c153f4293085ee0bd5fe97f100566df0b89aec4d23"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
            inner:   TokenInfo { decimals: 18, symbol: "3Crv".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            inner:   TokenInfo { decimals: 18, symbol: "STBT".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2MetaPool,
            trace_index: 1,
            from: Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            recipient: Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            pool: Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token_in,
            amount_in: U256::from_str("754647000000000000000000")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("770465351189286428927839")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
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

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_exchange1() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2MetaPool,
            Address::new(hex!("400d4C984779A747462e88373c3fE369EF9F5b50")),
            Address::new(hex!("c56c2b7e71B54d38Aab6d52E94a04Cbfa8F604fA")),
            Some(Address::new(hex!("853d955aCEf822Db058eb8505911ED77F175b99e"))),
            Some(Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"))),
            None,
            None,
            Some(Address::new(hex!("3175Df0976dFA876431C2E9eE6Bc45b65d3473CC"))),
        );

        let swap =
            B256::from(hex!("b457e8feea90502f81cd3326009069fe0ebe7409ae02a23d32c5edebc3314a6b"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("c56c2b7e71B54d38Aab6d52E94a04Cbfa8F604fA")),
            inner:   TokenInfo { decimals: 6, symbol: "ZUSD".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("3175Df0976dFA876431C2E9eE6Bc45b65d3473CC")),
            inner:   TokenInfo { decimals: 18, symbol: "crvFRAX".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2MetaPool,
            trace_index: 1,
            from: Address::new(hex!("C691A3446527899C2B063163F28bF15e3c18b50A")),
            recipient: Address::new(hex!("C691A3446527899C2B063163F28bF15e3c18b50A")),
            pool: Address::new(hex!("400d4C984779A747462e88373c3fE369EF9F5b50")),
            token_in,
            amount_in: U256::from_str("5").unwrap().to_scaled_rational(6),
            token_out,
            amount_out: U256::from_str("4991304502969")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
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

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_exchange_underlying0() {
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

        let swap =
            B256::from(hex!("248b8f2c6b80b138bcaeb53a4a2aea7f4dbc397313a887682cddf2909b676072"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            inner:   TokenInfo { decimals: 18, symbol: "STBT".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")),
            inner:   TokenInfo { decimals: 6, symbol: "USDT".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2MetaPool,
            trace_index: 1,
            from: Address::new(hex!("de7976D00607032445A79DC8aBe6d5b242705C1a")),
            recipient: Address::new(hex!("de7976D00607032445A79DC8aBe6d5b242705C1a")),
            pool: Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token_in,
            amount_in: U256::from_str("5000000000000000000")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("4987470").unwrap().to_scaled_rational(6),
            msg_value: U256::ZERO,
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

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_exchange_underlying1() {
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

        let swap =
            B256::from(hex!("a835d77e510a6218199c44aa911ac0056ebbb339015c3a0d56c4020c5ca5a115"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
            inner:   TokenInfo { decimals: 6, symbol: "USDC".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            inner:   TokenInfo { decimals: 18, symbol: "STBT".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2MetaPool,
            trace_index: 1,
            from: Address::new(hex!("31b8939c6e55a4ddaf0d6479320a0dfd9766ee9d")),
            recipient: Address::new(hex!("31b8939c6e55a4ddaf0d6479320a0dfd9766ee9d")),
            pool: Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token_in,
            amount_in: U256::from_str("500000000").unwrap().to_scaled_rational(6),
            token_out,
            amount_out: U256::from_str("500390219856882922498")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
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
