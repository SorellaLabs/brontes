use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

// couldn't find a V1 metapool calling this
action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::exchange_0Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplExchange_0CallLogs,
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
            trace_index: info.trace_idx,
            pool: info.from_address,
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
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::exchange_1Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplExchange_1CallLogs,
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
            trace_index: info.trace_idx,
            pool: info.from_address,
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

// couldn't find a V1 metapool calling this
action_impl!(
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::exchange_underlying_0Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplExchange_underlying_0CallLogs,
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
            trace_index: info.trace_idx,
            pool: info.from_address,
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
    Protocol::CurveV1MetapoolImpl,
    crate::CurveV1MetapoolImpl::exchange_underlying_1Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetapoolImplExchange_underlying_1CallLogs,
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
            trace_index: info.trace_idx,
            pool: info.from_address,
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
    async fn test_curve_v1_metapool_exchange0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV1MetaPool,
            Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            Some(Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F"))),
            Some(Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"))),
            Some(Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"))),
            None,
            Some(Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490"))),
        );

        let swap =
            B256::from(hex!("0c673f1ede30f20bb7ca3e7c05a71dcc49a8bb18498e148e3967bb7173d6794e"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            inner:   TokenInfo { decimals: 18, symbol: "T".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("6c3f90f043a72fa612cbac8115ee7e52bde6e490")),
            inner:   TokenInfo { decimals: 18, symbol: "3Crv".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: 1,
            from: Address::new(hex!("41ce1Af5B4eF2E124028dea59580817898def508")),
            recipient: Address::new(hex!("41ce1Af5B4eF2E124028dea59580817898def508")),
            pool: Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            token_in,
            amount_in: U256::from_str("108987327295834489846250")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("846346204017353217859")
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
            .unwrap()
    }

    #[brontes_macros::test]
    async fn test_curve_v1_metapool_exchange_underlying0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV1MetaPool,
            Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            Some(Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F"))),
            Some(Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"))),
            Some(Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"))),
            None,
            Some(Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490"))),
        );

        let swap =
            B256::from(hex!("84d55076a9eb3d9d30e26dc1f498f03216d7216647a51597933901c55534e355"));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("6967299e9F3d5312740Aa61dEe6E9ea658958e31")),
            inner:   TokenInfo { decimals: 18, symbol: "T".to_string() },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")),
            inner:   TokenInfo { decimals: 18, symbol: "DAI".to_string() },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Action::Swap(NormalizedSwap {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: 1,
            from: Address::new(hex!("A24AD612C61076C902588C28e617461c6cA1eD54")),
            recipient: Address::new(hex!("A24AD612C61076C902588C28e617461c6cA1eD54")),
            pool: Address::new(hex!("A77d09743F77052950C4eb4e6547E9665299BecD")),
            token_in,
            amount_in: U256::from_str("632358995283130172936168")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("605569147106308061507")
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
