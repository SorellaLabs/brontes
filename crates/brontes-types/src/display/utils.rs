use std::fmt;

use alloy_primitives::{Address, FixedBytes, B256};
use colored::{ColoredString, Colorize};
use indoc::indoc;
use itertools::Itertools;
use prettytable::{Cell, Row, Table};

use crate::{
    mev::{ArbDetails, AtomicArbType, Bundle, BundleData, CexDex, OptimisticTrade},
    utils::ToFloatNearest,
};
pub fn display_sandwich(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"

         _____                 _          _      _
        /  ___|               | |        (_)    | |
        \ `--.  __ _ _ __   __| |_      ___  ___| |__
         `--. \/ _` | '_ \ / _` \ \ /\ / / |/ __| '_ \
        /\__/ / (_| | | | | (_| |\ V  V /| | (__| | | |
        \____/ \__,_|_| |_|\__,_| \_/\_/ |_|\___|_| |_|

    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let sandwich_data = match &bundle.data {
        BundleData::Sandwich(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // MEV Bot Details
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    writeln!(f, "\n{}:", "Attacks".bright_yellow().underline())?;
    for (i, ((tx_hash, swaps), gas_details)) in sandwich_data
        .frontrun_tx_hash
        .iter()
        .zip(sandwich_data.frontrun_swaps.iter())
        .zip(sandwich_data.frontrun_gas_details.iter())
        .enumerate()
    {
        writeln!(
            f,
            "\n    {}: {}",
            format!("Frontrun {}", i + 1)
                .bright_blue()
                .bold()
                .underline(),
            format_etherscan_url(tx_hash)
        )?;

        // Frontrun swaps
        writeln!(f, "      - {}:", "Swaps".bright_blue())?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "            {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }

        // Frontrun gas details
        writeln!(f, "      - {}:", "Gas details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 12)?;

        // Victims of this frontrun transaction
        writeln!(f, "\n    {}:", "Victims".bright_red().bold().underline())?;
        if let Some(victim_tx_hashes) = sandwich_data.victim_swaps_tx_hashes.get(i) {
            for (k, tx_hash) in victim_tx_hashes.iter().enumerate() {
                let victim_swaps = sandwich_data.victim_swaps.get(k); // Assuming this matches victims to frontruns directly
                let victim_gas_details = sandwich_data.victim_swaps_gas_details.get(k); // Same assumption as above

                writeln!(
                    f,
                    "\n        {}: {}",
                    format!("Victim {}", k + 1).bright_red().bold(),
                    format_etherscan_url(tx_hash)
                )?;

                // Victim swaps
                writeln!(f, "          - {}:", "Swaps".bright_blue())?;
                if let Some(swaps) = victim_swaps {
                    for (l, swap) in swaps.iter().enumerate() {
                        writeln!(
                            f,
                            "                {}: {}",
                            format!(" - {}", l + 1).green(),
                            swap
                        )?;
                    }
                }

                // Victim gas details
                writeln!(f, "          - {}:", "Gas details".bright_blue())?;
                if let Some(gas_details) = victim_gas_details {
                    gas_details.pretty_print_with_spaces(f, 16)?;
                }
            }
        }
    }

    // Backrun Section
    writeln!(f, "\n{}:\n", "Backrun Transaction".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Transaction".bright_blue(),
        format_etherscan_url(&sandwich_data.backrun_tx_hash)
    )?;

    writeln!(f, "     - {}:", "Swaps".bright_blue())?;
    for (i, swap) in sandwich_data.backrun_swaps.iter().enumerate() {
        writeln!(f, "        {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }

    writeln!(f, "     - {}:", "Gas Details".bright_blue())?;
    sandwich_data
        .backrun_gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Profitability Section
    writeln!(f, "\n{}\n", "Profitability".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Bundle Profit (USD)".bright_white(),
        format_profit(bundle.header.profit_usd)
            .to_string()
            .bright_white()
    )?;
    writeln!(
        f,
        " - {}: {}",
        "Bribe (USD)".bright_white(),
        format_bribe(bundle.header.bribe_usd)
            .to_string()
            .bright_red()
    )?;

    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));

    Ok(())
}

pub fn display_jit_liquidity_sandwich(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"
           ___ _ _          _____                 _          _      _
          |_  (_) |        /  ___|               | |        (_)    | |
            | |_| |_ ______\ `--.  __ _ _ __   __| |_      ___  ___| |__
            | | | __|______|`--. \/ _` | '_ \ / _` \ \ /\ / / |/ __| '_ \
        /\__/ / | |_       /\__/ / (_| | | | | (_| |\ V  V /| | (__| | | |
        \____/|_|\__|      \____/ \__,_|_| |_|\__,_| \_/\_/ |_|\___|_| |_|

    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let jit_sandwich_data = match &bundle.data {
        BundleData::JitSandwich(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // MEV Bot Details
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    // Frontrun Section
    writeln!(f, "\n{}:", "Attacks".bright_yellow().underline())?;
    for (i, (((tx_hash, swaps), mints), gas_details)) in jit_sandwich_data
        .frontrun_tx_hash
        .iter()
        .zip(jit_sandwich_data.frontrun_swaps.iter())
        .zip(jit_sandwich_data.frontrun_mints.iter())
        .zip(jit_sandwich_data.frontrun_gas_details.iter())
        .enumerate()
    {
        writeln!(
            f,
            "\n    {}: {}",
            format!("Frontrun {}", i + 1)
                .bright_blue()
                .bold()
                .underline(),
            format_etherscan_url(tx_hash)
        )?;

        // Frontrun swaps
        writeln!(f, "      - {}:", "Swaps".bright_blue())?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "            {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }

        // Frontrun mints
        if let Some(ref mints) = mints {
            if !mints.is_empty() {
                writeln!(f, "      - {}:", "Mints".bright_blue())?;
                for (j, mint) in mints.iter().enumerate() {
                    writeln!(f, "            {}: {}", format!(" - {}", j + 1).green(), mint)?;
                }
            }
        }

        // Frontrun gas details
        writeln!(f, "      - {}:", "Gas details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 12)?;

        // Victims of this frontrun transaction
        writeln!(f, "\n    {}:", "Victims".bright_red().bold().underline())?;
        if let Some(victim_tx_hashes) = jit_sandwich_data.victim_swaps_tx_hashes.get(i) {
            for (k, tx_hash) in victim_tx_hashes.iter().enumerate() {
                let victim_swaps = jit_sandwich_data.victim_swaps.get(k);
                let victim_gas_details = jit_sandwich_data.victim_swaps_gas_details.get(k);

                writeln!(
                    f,
                    "\n        {}: {}",
                    format!("Victim {}", k + 1).bright_red().bold(),
                    format_etherscan_url(tx_hash)
                )?;

                // Victim swaps
                writeln!(f, "          - {}:", "Swaps".bright_blue())?;
                if let Some(swaps) = victim_swaps {
                    for (l, swap) in swaps.iter().enumerate() {
                        writeln!(
                            f,
                            "                {}: {}",
                            format!(" - {}", l + 1).green(),
                            swap
                        )?;
                    }
                }

                writeln!(f, "          - {}:", "Gas details".bright_blue())?;
                if let Some(gas_details) = victim_gas_details {
                    gas_details.pretty_print_with_spaces(f, 16)?;
                }
            }
        }
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Transaction".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Backrun Transaction".bright_blue(),
        format_etherscan_url(&jit_sandwich_data.backrun_tx_hash)
    )?;

    writeln!(f, "     - {}:", "Actions".bright_blue())?;
    for (i, swap) in jit_sandwich_data.backrun_swaps.iter().enumerate() {
        writeln!(f, "      {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }

    for (i, burn) in jit_sandwich_data.backrun_burns.iter().enumerate() {
        writeln!(f, "      {}: {}", format!(" - {}", i + 1).green(), burn)?;
    }

    writeln!(f, "     - {}:", "Gas Details".bright_blue())?;
    jit_sandwich_data
        .backrun_gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Profitability Section
    writeln!(f, "\n{}\n", "Profitability".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Bundle Profit (USD)".bright_white(),
        format_profit(bundle.header.profit_usd)
            .to_string()
            .bright_white()
    )?;
    writeln!(
        f,
        " - {}: {}",
        "Bribe (USD)".bright_white(),
        format_bribe(bundle.header.bribe_usd)
            .to_string()
            .bright_red()
    )?;

    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));

    Ok(())
}

const STABLE_COIN_HEADER: &str = indoc! {r#"
 _____ _        _     _                _          ___       _     
/  ___| |      | |   | |              (_)        / _ \     | |    
\ `--.| |_ __ _| |__ | | ___  ___ ___  _ _ __   / /_\ \_ __| |__  
 `--. \ __/ _` | '_ \| |/ _ \/ __/ _ \| | '_ \  |  _  | '__| '_ \ 
/\__/ / || (_| | |_) | |  __/ (_| (_) | | | | | | | | | |  | |_) |
\____/ \__\__,_|_.__/|_|\___|\___\___/|_|_| |_| \_| |_/_|  |_.__/                                                       
                                                                
"#};

const LONG_TAIL_HEADER: &str = indoc! {r#"
 _                         _______    _ _                 _     
| |                       |__   __|  (_) |     /\        | |    
| |     ___  _ __   __ _     | | __ _ _| |    /  \   _ __| |__  
| |    / _ \| '_ \ / _` |    | |/ _` | | |   / /\ \ | '__| '_ \ 
| |___| (_) | | | | (_| |    | | (_| | | |  / ____ \| |  | |_) |
|______\___/|_| |_|\__, |    |_|\__,_|_|_| /_/    \_\_|  |_.__/ 
                    __/ |                                       
                   |___/                                        
"#};

const CROSS_PAIR: &str = indoc! {r#"
 _____                    ______     _         ___       _     
/  __ \                   | ___ \   (_)       / _ \     | |    
| /  \/_ __ ___  ___ ___  | |_/ /_ _ _ _ __  / /_\ \_ __| |__  
| |   | '__/ _ \/ __/ __| |  __/ _` | | '__| |  _  | '__| '_ \ 
| \__/\ | | (_) \__ \__ \ | | | (_| | | |    | | | | |  | |_) |
 \____/_|  \___/|___/___/ \_|  \__,_|_|_|    \_| |_/_|  |_.__/ 
                                                               
"#};

const TRIANGULAR_ARB: &str = indoc! {r#"
 _____    _                         _               ___       _     
|_   _|  (_)                       | |             / _ \     | |    
  | |_ __ _  __ _ _ __   __ _ _   _| | __ _ _ __  / /_\ \_ __| |__  
  | | '__| |/ _` | '_ \ / _` | | | | |/ _` | '__| |  _  | '__| '_ \ 
  | | |  | | (_| | | | | (_| | |_| | | (_| | |    | | | | |  | |_) |
  \_/_|  |_|\__,_|_| |_|\__, |\__,_|_|\__,_|_|    \_| |_/_|  |_.__/ 
                         __/ |                                      
                        |___/                                       
"#};

pub fn display_atomic_backrun(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let atomic_backrun_data = match &bundle.data {
        BundleData::AtomicArb(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    match atomic_backrun_data.arb_type {
        AtomicArbType::Triangle => {
            for line in TRIANGULAR_ARB.lines() {
                writeln!(f, "{}", line.bright_yellow())?;
            }
        }
        AtomicArbType::CrossPair(_) => {
            for line in CROSS_PAIR.lines() {
                writeln!(f, "{}", line.bright_yellow())?;
            }
        }
        AtomicArbType::StablecoinArb => {
            for line in STABLE_COIN_HEADER.lines() {
                writeln!(f, "{}", line.bright_blue())?;
            }
        }
        AtomicArbType::LongTail => {
            for line in LONG_TAIL_HEADER.lines() {
                writeln!(f, "{}", line.bright_green())?;
            }
        }
    }

    // Tx details
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    let tx_url = format!("https://etherscan.io/tx/{:?}", bundle.header.tx_hash).underline();
    writeln!(f, "   - Etherscan: {}", tx_url)?;

    if atomic_backrun_data.trigger_tx != B256::ZERO {
        let tx_url =
            format!("https://etherscan.io/tx/{:?}", atomic_backrun_data.trigger_tx).underline();
        writeln!(f, "   - Trigger Tx: {}", tx_url)?;
    }

    // Arb Section
    writeln!(
        f,
        "\n{}\n",
        atomic_backrun_data
            .arb_type
            .to_string()
            .bright_yellow()
            .underline()
    )?;
    writeln!(f, " - {}", "Swaps:".bright_blue())?;
    for (i, swap) in atomic_backrun_data.swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }

    writeln!(f, " - {}:", "Gas Details".bright_blue())?;
    atomic_backrun_data
        .gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Profitability Section
    writeln!(f, "\n{}\n", "Profitability".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Bundle Profit (USD)".bright_white(),
        format_profit(bundle.header.profit_usd)
            .to_string()
            .bright_white()
    )?;
    writeln!(
        f,
        " - {}: {}",
        "Bribe (USD)".bright_white(),
        format_bribe(bundle.header.bribe_usd)
            .to_string()
            .bright_red()
    )?;

    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));

    Ok(())
}

pub fn display_liquidation(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"

         _     _             _     _       _   _
        | |   (_)           (_)   | |     | | (_)
        | |    _  __ _ _   _ _  __| | __ _| |_ _  ___  _ __
        | |   | |/ _` | | | | |/ _` |/ _` | __| |/ _ \| '_ \
        | |___| | (_| | |_| | | (_| | (_| | |_| | (_) | | | |
        \_____/_|\__, |\__,_|_|\__,_|\__,_|\__|_|\___/|_| |_|
                    | |
                    |_|

    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let liquidation_data = match &bundle.data {
        BundleData::Liquidation(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // MEV Bot Details
    writeln!(f, "\n{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    // Liquidation Transaction Section
    writeln!(f, "{}\n", "Liquidation".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Transaction".bright_blue(),
        format_etherscan_url(&liquidation_data.liquidation_tx_hash)
    )?;
    writeln!(
        f,
        " - {}: {}",
        "Trigger".bright_blue(),
        format_etherscan_url(&liquidation_data.trigger)
    )?;

    // Swaps Section
    writeln!(f, "\n{}\n", "Liquidation Swaps".bright_yellow().underline())?;
    for (i, swap) in liquidation_data.liquidation_swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }

    // Liquidations Section
    writeln!(f, "\n{}\n", "Liquidations".bright_yellow().underline())?;
    for (i, liquidation) in liquidation_data.liquidations.iter().enumerate() {
        writeln!(f, " - {}:", format!("Liquidation {}", i + 1).bright_blue())?;
        liquidation.pretty_print(f, 8)?;
    }

    // Gas Details Section
    writeln!(f, "\n - {}:", "Gas Details".bright_blue())?;
    liquidation_data
        .gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Profitability Section
    writeln!(f, "\n{}\n", "Profitability".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Bundle Profit (USD)".bright_white(),
        format_profit(bundle.header.profit_usd)
            .to_string()
            .bright_white()
    )?;
    writeln!(
        f,
        " - {}: {}\n",
        "Bribe (USD)".bright_white(),
        format_bribe(bundle.header.bribe_usd)
            .to_string()
            .bright_red()
    )?;

    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));
    Ok(())
}

pub fn display_jit_liquidity(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"

           ___ _ _          _     _             _     _ _ _
          |_  (_) |        | |   (_)           (_)   | (_) |
            | |_| |_ ______| |    _  __ _ _   _ _  __| |_| |_ _   _
            | | | __|______| |   | |/ _` | | | | |/ _` | | __| | | |
        /\__/ / | |_       | |___| | (_| | |_| | | (_| | | |_| |_| |
        \____/|_|\__|      \_____/_|\__, |\__,_|_|\__,_|_|\__|\__, |
                                       | |                     __/ |
                                       |_|                    |___/

    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let jit_data = match &bundle.data {
        BundleData::Jit(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // MEV Bot Details
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    // Frontrun Section
    writeln!(f, "\n{}\n", "Frontrun Mints".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Mint Transaction".bright_blue(),
        format_etherscan_url(&jit_data.frontrun_mint_tx_hash)
    )?;
    writeln!(f, " - {}", "Mints:".bright_blue())?;
    for (i, mint) in jit_data.frontrun_mints.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), mint)?;
    }
    writeln!(f, " - {}:", "Gas Details".bright_blue())?;
    jit_data
        .frontrun_mint_gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Victim Section
    writeln!(f, "\n{}\n", "Victim Transactions".bright_yellow().underline())?;
    for (i, tx_hash) in jit_data.victim_swaps_tx_hashes.iter().enumerate() {
        let swaps = &jit_data.victim_swaps[i];
        let gas_details = &jit_data.victim_swaps_gas_details[i];

        writeln!(
            f,
            " - {}: {}",
            format!("Victim Transaction {}", i + 1).bright_magenta(),
            format_etherscan_url(tx_hash)
        )?;

        writeln!(f, "     - {}:", "Swaps".bright_blue())?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "      {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }

        writeln!(f, "     - {}:", "Gas Details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 8)?;
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Burns".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Burn Transaction".to_string().bright_magenta(),
        format_etherscan_url(&jit_data.backrun_burn_tx_hash)
    )?;

    writeln!(f, " - {}", "Burns:".bright_blue())?;
    for (i, burn) in jit_data.backrun_burns.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), burn)?;
    }
    writeln!(f, " - {}:", "Gas Details".bright_blue())?;
    jit_data
        .backrun_burn_gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Profitability Section
    writeln!(f, "\n{}\n", "Profitability".bright_yellow().underline())?;
    writeln!(
        f,
        " - {}: {}",
        "Bundle Profit (USD)".bright_white(),
        format_profit(bundle.header.profit_usd)
            .to_string()
            .bright_white()
    )?;
    writeln!(
        f,
        " - {}: {}",
        "Bribe (USD)".bright_white(),
        format_bribe(bundle.header.bribe_usd)
            .to_string()
            .bright_red()
    )?;

    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));

    Ok(())
}

pub fn display_cex_dex(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"

             _____ _        _           ___       _
            /  ___| |      | |         / _ \     | |
            \ `--.| |_ __ _| |_ ______/ /_\ \_ __| |__
             `--. \ __/ _` | __|______|  _  | '__| '_ \
            /\__/ / || (_| | |_       | | | | |  | |_) |
            \____/ \__\__,_|\__|      \_| |_/_|  |_.__/


        "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.purple())?;
    }

    let cex_dex_data = match &bundle.data {
        BundleData::CexDex(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // Tx details
    writeln!(f, "\n{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", contract)?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    writeln!(f, "   - Etherscan: {}", format_etherscan_url(&bundle.header.tx_hash))?;

    // Mev section
    writeln!(f, "\n{}", "MEV:\n".bold().underline().bright_yellow())?;
    writeln!(f, "   - Max Profit Route (USD): {}", format_profit(bundle.header.profit_usd))?;
    writeln!(
        f,
        "   - Max Profit Methodology: {}",
        cex_dex_data.header_pnl_methodology.to_string().red()
    )?;
    writeln!(f, "   - Bribe (USD): {}", (format_bribe(bundle.header.bribe_usd)).to_string().red())?;

    writeln!(f, "Block Timestamp:\n {}", cex_dex_data.block_timestamp)?;
    // Cex-dex specific details
    writeln!(f, "\n{}", "Cex-Dex Details:\n".bold().bright_yellow().underline())?;

    writeln!(f, "  - {}:", "PnL".bright_blue())?;

    writeln!(f, "  - {}: Global VMAP PnL", "PnL".bright_blue())?;
    writeln!(
        f,
        "    - Maker: {:.6}, Taker: {:.6}",
        cex_dex_data.global_vmap_pnl_maker.clone().to_float(),
        cex_dex_data.global_vmap_pnl_taker.clone().to_float()
    )?;
    writeln!(f, "  - {}: Optimistic PnL", "PnL".bright_yellow())?;
    writeln!(
        f,
        "    - Maker: {:.6}, Taker: {:.6}",
        cex_dex_data.optimistic_route_pnl_maker.clone().to_float(),
        cex_dex_data.optimistic_route_pnl_taker.clone().to_float()
    )?;

    display_optimistic_trades(f, cex_dex_data)?;

    writeln!(f, "  - {}: Optimal Route PnL", "PnL".bright_blue())?;
    writeln!(
        f,
        "    - Maker: {:.6}, Taker: {:.6}",
        cex_dex_data.optimal_route_pnl_maker.clone().to_float(),
        cex_dex_data.optimal_route_pnl_taker.clone().to_float()
    )?;

    writeln!(f, "  - {}", "Per Exchange PnL:".bold().underline().purple())?;
    for (exchange, pnl) in &cex_dex_data.per_exchange_pnl {
        writeln!(f, "    - {}:", exchange.to_string().bold().underline().green())?;
        writeln!(
            f,
            "      - Maker: {:.6} Taker: {:.6}",
            pnl.0.clone().to_float(),
            pnl.1.clone().to_float()
        )?;
    }

    writeln!(f, "\n----------------------------------------")?;
    writeln!(f, "{}", "Arb Details".bold().red().underline())?;

    for (i, swap) in cex_dex_data.swaps.iter().enumerate() {
        writeln!(f, "\n{}: - {}", format!("Swap {}", i + 1).bold().blue().underline(), swap)?;

        writeln!(f, "   - {}:", "Max Profit Route".purple().bold().underline())?;
        if i < cex_dex_data.optimal_route_details.len() {
            display_arb_details(f, &cex_dex_data.optimal_route_details[i])?;
        } else {
            writeln!(f, "   - Error: No optimal route detail available for swap {}", i + 1)?;
        }

        writeln!(f, "   - {}:", "Global VMAP".purple().bold().underline())?;
        if i < cex_dex_data.global_vmap_details.len() {
            display_arb_details(f, &cex_dex_data.global_vmap_details[i])?;
        } else {
            writeln!(f, "   - Error: No global VMAP detail available for swap {}", i + 1)?;
        }

        writeln!(f, "   {}:", "Per Exchange Arb Details".purple().bold().underline())?;
        if i < cex_dex_data.per_exchange_details.len() {
            for details in cex_dex_data.per_exchange_details[i].iter() {
                display_arb_details(f, details)?;
            }
        } else {
            writeln!(f, "   - Error: No per exchange arb details available for swap {}", i + 1)?;
        }
    }

    // Gas Details
    writeln!(f, "\n{}: \n", "Gas Details".underline().bright_yellow())?;

    cex_dex_data.gas_details.pretty_print_with_spaces(f, 8)?;

    bundle.header.balance_deltas.iter().for_each(|tx_delta| {
        writeln!(f, "\n\n{}", tx_delta).expect("Failed to write balance deltas")
    });

    Ok(())
}
fn display_arb_details(f: &mut fmt::Formatter<'_>, details: &ArbDetails) -> fmt::Result {
    writeln!(f, "     Pairs: {:?}", details.pairs)?;
    writeln!(f, "     Trade Window: {} - {}", details.trade_start_time, details.trade_end_time)?;
    writeln!(f, "     CEX Exchange: {:?}", details.cex_exchange)?;
    writeln!(
        f,
        "     Price (Maker/Taker): {:.8} / {:.8}",
        details.price_maker.clone().to_float(),
        details.price_taker.clone().to_float()
    )?;
    writeln!(f, "     DEX Exchange: {:?}", details.dex_exchange)?;
    writeln!(f, "     DEX Price: {:.8}", details.dex_price.clone().to_float())?;
    writeln!(f, "     DEX Amount: {:.8}", details.dex_amount.clone().to_float())?;
    writeln!(
        f,
        "     PnL (Maker/Taker): {:.8} / {:.8}",
        details.pnl_maker.clone().to_float(),
        details.pnl_taker.clone().to_float()
    )?;
    Ok(())
}

pub fn display_optimistic_trades(
    f: &mut std::fmt::Formatter<'_>,
    cex_dex_data: &CexDex,
) -> std::fmt::Result {
    writeln!(f, "  - {}: Optimistic Route PnL", "PnL".bright_blue())?;
    writeln!(
        f,
        "    - Maker: {:.8}, Taker: {:.8}",
        cex_dex_data.optimistic_route_pnl_maker.clone().to_float(),
        cex_dex_data.optimistic_route_pnl_taker.clone().to_float(),
    )?;
    if !cex_dex_data.optimistic_trade_details.is_empty() {
        writeln!(f, "\n  - {}: Optimistic Trade Details", "Trades".bright_green())?;
        let mut table = Table::new();
        table.add_row(Row::new(vec![
            Cell::new("Exchange").style_spec("Fb"),
            Cell::new("Pair").style_spec("Fb"),
            Cell::new("Time from block (ms)").style_spec("Fb"),
            Cell::new("Price").style_spec("Fb"),
            Cell::new("Volume").style_spec("Fb"),
        ]));

        let all_trades: Vec<&OptimisticTrade> = cex_dex_data
            .optimistic_trade_details
            .iter()
            .flatten()
            .sorted_by_key(|trade| trade.timestamp)
            .collect();

        let merged_trades = merge_trades(all_trades, 7); // Cap at 7 entries

        for trade in merged_trades {
            let relative_time =
                (trade.timestamp as i64 - cex_dex_data.block_timestamp as i64) / 1000;
            table.add_row(Row::new(vec![
                Cell::new(&format!("{:?}", trade.exchange)),
                Cell::new(&format!("{:?}", trade.pair)),
                Cell::new(&format!("{}", relative_time)),
                Cell::new(&format!("{:.8}", trade.price.clone().to_float())),
                Cell::new(&format!("{:.8}", trade.volume.clone().to_float())),
            ]));
        }
        write!(f, "{}", table)?;
    }
    Ok(())
}

fn merge_trades(trades: Vec<&OptimisticTrade>, max_entries: usize) -> Vec<OptimisticTrade> {
    if trades.len() <= max_entries {
        return trades.into_iter().cloned().collect();
    }

    let mut merged = Vec::new();
    let chunk_size = (trades.len() as f32 / max_entries as f32).ceil() as usize;

    for chunk in trades.chunks(chunk_size) {
        let mut merged_trade = chunk[0].clone();
        for trade in chunk.iter().skip(1) {
            merged_trade.volume += trade.volume.clone();
            merged_trade.price = (merged_trade.price.clone() * merged_trade.volume.clone()
                + trade.price.clone() * trade.volume.clone())
                / (merged_trade.volume.clone() + trade.volume.clone());
        }
        merged_trade.exchange = format!("{:?}+", merged_trade.exchange).into();
        merged.push(merged_trade);
    }

    merged
}

pub fn display_cex_dex_quotes(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"
        ██████╗███████╗██╗  ██╗    ██████╗ ███████╗██╗  ██╗
        ██╔════╝██╔════╝╚██╗██╔╝    ██╔══██╗██╔════╝╚██╗██╔╝
        ██║     █████╗   ╚███╔╝     ██║  ██║█████╗   ╚███╔╝ 
        ██║     ██╔══╝   ██╔██╗     ██║  ██║██╔══╝   ██╔██╗ 
        ╚██████╗███████╗██╔╝ ██╗    ██████╔╝███████╗██╔╝ ██╗
        ╚═════╝╚══════╝╚═╝  ╚═╝    ╚═════╝ ╚══════╝╚═╝  ╚═╝
    "#};

    writeln!(f, "{}", ascii_header.purple())?;

    let cex_dex_data = match &bundle.data {
        BundleData::CexDexQuote(data) => data,
        _ => return Err(fmt::Error),
    };

    writeln!(f, "\n{}", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Tx Hash: {}", format_etherscan_url(&bundle.header.tx_hash))?;
    writeln!(f, "   - Block Number: {}", bundle.header.block_number)?;
    writeln!(f, "   - Block Timestamp: {}", cex_dex_data.block_timestamp)?;
    writeln!(f, "   - Bribe USD: {}", bundle.header.bribe_usd)?;

    writeln!(f, "\n{}", "Quote Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Exchange: {}", cex_dex_data.exchange.to_string().green())?;
    writeln!(f, "   - PnL (USD): {}", format!("{:.6}", cex_dex_data.pnl).cyan())?;

    writeln!(f, "\n{}", "Swaps".bold().underline().bright_yellow())?;
    for (i, swap) in cex_dex_data.swaps.iter().enumerate() {
        writeln!(f, "   Swap {}: {}", i + 1, swap)?;
        if i < cex_dex_data.instant_mid_price.len() {
            writeln!(f, "      - Mid Price: {:.6}", cex_dex_data.instant_mid_price[i])?;
        }
        if i < cex_dex_data.t2_mid_price.len() {
            writeln!(f, "      - Mid Price (T2): {:.6}", cex_dex_data.t2_mid_price[i])?;
        }

        if i < cex_dex_data.t12_mid_price.len() {
            writeln!(f, "      - Mid Price (T12): {:.6}", cex_dex_data.t12_mid_price[i])?;
        }

        if i < cex_dex_data.t60_mid_price.len() {
            writeln!(f, "      - Mid Price (T60): {:.6}", cex_dex_data.t60_mid_price[i])?;
        }

        if i < cex_dex_data.t300_mid_price.len() {
            writeln!(f, "      - Mid Price (T300): {:.6}", cex_dex_data.t300_mid_price[i])?;
        }
    }
    // Gas Details
    writeln!(f, "\n{}: \n", "Gas Details".underline().bright_yellow())?;

    cex_dex_data.gas_details.pretty_print_with_spaces(f, 8)?;

    Ok(())
}

pub fn display_searcher_tx(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"
    
             _____                     _                 _____    
            /  ___|                   | |               |_   _|   
            \ `--.  ___  __ _ _ __ ___| |__   ___ _ __    | |_  __
             `--. \/ _ \/ _` | '__/ __| '_ \ / _ \ '__|   | \ \/ /
            /\__/ /  __/ (_| | | | (__| | | |  __/ |      | |>  < 
            \____/ \___|\__,_|_|  \___|_| |_|\___|_|      \_/_/\_\

    "#};

    let searcher_tx_data = match &bundle.data {
        BundleData::Unknown(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    for line in ascii_header.lines() {
        writeln!(f, "{}", line)?;
    }

    // Tx details
    writeln!(f, "\n{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;

    match bundle.header.mev_contract {
        Some(contract) => {
            writeln!(f, "   - Mev Contract: {}", formate_etherscan_address_url(&contract))?;
        }
        None => {
            writeln!(f, "   - Mev Contract: None")?;
        }
    }

    writeln!(f, "   - Etherscan: {}", format_etherscan_url(&bundle.header.tx_hash))?;

    writeln!(f, "  - {}:", "PnL".bright_blue())?;

    writeln!(f, "   - Transaction Profit (USD): {}", format_profit(bundle.header.profit_usd))?;
    writeln!(f, "   - Bribe (USD): {}", (format_bribe(bundle.header.bribe_usd)).to_string().red())?;

    // Transfers
    bundle
        .header
        .balance_deltas
        .iter()
        .for_each(|tx_delta| writeln!(f, "{}", tx_delta).expect("Failed to write balance deltas"));

    // Gas Details
    writeln!(f, "\n{}: \n", "Gas Details".underline().bright_yellow())?;

    searcher_tx_data
        .gas_details
        .pretty_print_with_spaces(f, 8)?;

    Ok(())
}

// Helper function to format profit values
fn format_profit(value: f64) -> ColoredString {
    if value < 0.0 {
        format!("-${:.2}", value.abs()).red()
    } else if value > 0.0 {
        format!("${:.2}", value).green()
    } else {
        format!("${:.2}", value).white()
    }
}

fn format_bribe(value: f64) -> ColoredString {
    format!("${:.2}", value).red()
}

pub fn format_etherscan_url(tx_hash: &FixedBytes<32>) -> String {
    format!("https://etherscan.io/tx/{:?}", tx_hash)
        .underline()
        .to_string()
}

pub fn formate_etherscan_address_url(tx_hash: &Address) -> String {
    format!("https://etherscan.io/address/{:?}", tx_hash)
        .underline()
        .to_string()
}
