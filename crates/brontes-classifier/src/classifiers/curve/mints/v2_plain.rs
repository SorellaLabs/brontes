use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedMint, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveV2PlainPoolImpl,
    crate::CurveV2PlainImpl::add_liquidity_0Call,
    Mint,
    [..AddLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplAdd_liquidity_0CallLogs,
    db_tx: &DB|{
        let log = log.add_liquidity_field?;

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

        Ok(NormalizedMint {
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
    crate::CurveV2PlainImpl::add_liquidity_1Call,
    Mint,
    [..AddLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2PlainPoolImplAdd_liquidity_1CallLogs,
    db_tx: &DB|{
        let log = log.add_liquidity_field?;

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

        Ok(NormalizedMint {
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
    async fn test_curve_v2_plain_pool_add_liquidity1() {
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

        let mint =
            B256::from(hex!("41abafaca09899889ef6d14d6aa95f00cd4558dce879f062dc55624994514329"));

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

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    Protocol::CurveV2PlainPool,
            trace_index: 1,
            from:        Address::new(hex!("fE894446bfaD2993B16428C990D69c99623b89B7")),
            recipient:   Address::new(hex!("fE894446bfaD2993B16428C990D69c99623b89B7")),
            pool:        Address::new(hex!("9D0464996170c6B9e75eED71c68B99dDEDf279e8")),
            token:       vec![token0, token1],
            amount:      vec![
                U256::from(2503890709681717311281_u128).to_scaled_rational(18),
                U256::from(798080784008874713734_u128).to_scaled_rational(18),
            ],
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_mint),
            )
            .await
            .unwrap();
    }
}
