use std::{
    collections::HashMap,
    fmt::{self, Debug, Display},
};

use alloy_primitives::Address;
use colored::Colorize;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

use super::MevType;
use crate::db::{
    redefined_types::primitives::*,
    token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq, Row, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct BundleHeader {
    pub block_number:  u64,
    pub tx_index:      u64,
    #[serde_as(as = "FixedString")]
    // For a sandwich this is always the first frontrun tx hash
    pub tx_hash: B256,
    #[serde_as(as = "FixedString")]
    pub eoa:           Address,
    #[serde_as(as = "FixedString")]
    pub mev_contract:  Address,
    pub profit_usd:    f64,
    pub token_profits: TokenProfits,
    pub bribe_usd:     f64,
    #[redefined(same_fields)]
    pub mev_type:      MevType,
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Row, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TokenProfits {
    pub profits: Vec<TokenProfit>,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TokenProfit {
    pub profit_collector: Address,
    pub token:            TokenInfoWithAddress,
    pub amount:           f64,
    pub usd_value:        f64,
}

impl TokenProfits {
    //TODO: Find is short circuiting, in this case this should be fine but not
    // entirely sure.
    pub fn compose(&mut self, to_compose: &TokenProfits) {
        for profit in &to_compose.profits {
            if let Some(existing_profit) = self
                .profits
                .iter_mut()
                .find(|p| p.profit_collector == profit.profit_collector && p.token == profit.token)
            {
                if existing_profit.amount < profit.amount {
                    existing_profit.amount = profit.amount;
                }
            }
        }
    }

    //TODO: Alternatively we could do something like this, but I'm not sure it's
    // even necessary

    /*
    pub fn compose(&mut self, to_compose: &TokenProfits) {
        for profit in &to_compose.profits {
            for existing_profit in self.profits.iter_mut().filter(|p|
                p.profit_collector == profit.profit_collector && p.token_address == profit.token_address
            ) {
                if existing_profit.amount < profit.amount {
                    existing_profit.amount = profit.amount;
                }
            }
        }
    }
     */

    pub fn print_with_labels(
        &self,
        f: &mut fmt::Formatter,
        labels: Option<&HashMap<Address, String>>,
    ) -> fmt::Result {
        writeln!(f, "\n{}", "Token Deltas:\n".bold().bright_white().underline())?;

        let mut profits_by_collector: HashMap<Address, Vec<&TokenProfit>> = HashMap::new();
        for profit in &self.profits {
            profits_by_collector
                .entry(profit.profit_collector.clone())
                .or_default()
                .push(profit);
        }

        for (collector, profits) in &profits_by_collector {
            let label = labels
                .and_then(|l| l.get(collector))
                .map_or_else(|| collector.to_string(), |l| l.clone());
            writeln!(f, " - {}: ", label.bright_white())?;

            for profit in profits {
                let amount_display = if profit.amount >= 0.0 {
                    format!("{:.7}", profit.amount).green()
                } else {
                    format!("{:.7}", profit.amount.abs()).red()
                };
                writeln!(
                    f,
                    "    {}: {} {} (worth ${:.3})",
                    if profit.amount >= 0.0 { "Gained" } else { "Lost" },
                    amount_display,
                    profit.token.inner.symbol.bold(),
                    profit.usd_value.abs()
                )?;
            }
        }

        Ok(())
    }
}

impl Display for TokenProfits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.print_with_labels(f, None)?;

        Ok(())
    }
}

impl Display for TokenProfit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (profit_or_loss, amount_str, usd_value_str) = if self.amount < 0.0 {
            ("lost", self.amount.to_string().red(), format!("$ {}", self.usd_value).red())
        } else {
            ("gained", self.amount.to_string().green(), format!("$ {}", self.usd_value).green())
        };

        writeln!(
            f,
            "Address: {} {} {} {} worth {}",
            self.profit_collector,
            profit_or_loss,
            amount_str,
            self.token.symbol.bold(),
            usd_value_str
        )
    }
}
