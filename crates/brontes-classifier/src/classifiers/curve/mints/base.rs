use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedMint, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool,
    crate::CurveBase::add_liquidityCall,
    Mint,
    [..AddLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePooladd_liquidityCallLogs,
    db_tx: &DB
    |{
        let log = log.AddLiquidity_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token_addrs = details.into_iter().collect::<Vec<_>>();
        let (tokens, amounts): (Vec<_>, Vec<_>) = log.token_amounts.into_iter().enumerate().map(|(i, a)|
        {
            let token = db_tx.try_fetch_token_info(token_addrs[i])?;
            let decimals = token.decimals;
            Ok((token, a.to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();


        Ok(NormalizedMint {
            protocol: Protocol::CurveBasePool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: amounts,
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
    async fn test_curve_base_add_liquidity() {
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

        let mint = B256::from(hex!(
            "dbf57244aad3402faa04e1ff19d3af0f89e1ac9aff3dd3830d2d6415b4dfdc0c"
        ));

        let token0 = TokenInfoWithAddress {
            address: Address::new(hex!("2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "WBTC".to_string(),
            },
        };

        let token1 = TokenInfoWithAddress {
            address: Address::new(hex!("EB4C2781e4ebA804CE9a9803C67d0893436bB27D")),
            inner: TokenInfo {
                decimals: 8,
                symbol: "renBTC".to_string(),
            },
        };

        classifier_utils.ensure_token(token0.clone());
        classifier_utils.ensure_token(token1.clone());

        let eq_action = Actions::Mint(NormalizedMint {
            protocol: Protocol::CurveBasePool,
            trace_index: 0,
            from: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            recipient: Address::new(hex!("0F5cd3C453A7FCD7735eB2f0493F36D41398A4a0")),
            pool: Address::new(hex!("7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714")),
            token: vec![token0, token1],
            amount: vec![
                U256::from(0).to_scaled_rational(8),
                U256::from(27506).to_scaled_rational(8),
                U256::from(0).to_scaled_rational(8),
            ],
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_mint())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_mint()),
        };

        classifier_utils
            .contains_action(mint, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
