use std::fmt;

use alloy_primitives::FixedBytes;
use colored::{ColoredString, Colorize};
use indoc::indoc;

use crate::mev::{Bundle, BundleData};

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

    // Frontrun Section
    writeln!(f, "{}\n", "Frontrun Transactions".bright_yellow().underline())?;
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
            " - {}: {}",
            format!("Transaction {}", i + 1).bright_blue(),
            format_etherscan_url(tx_hash)
        )?;
        writeln!(f, "     - {}:", "Actions".bright_blue())?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "      {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }

        if let Some(mint_list) = mints {
            let no_of_swaps: usize = swaps.len() + 1;
            for (j, mint) in mint_list.iter().enumerate() {
                writeln!(f, "      {}: {}", format!(" - {}", j + no_of_swaps).green(), mint)?;
            }
        }
        writeln!(f, "     - {}:", "Gas Details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 8)?;
    }

    // Victim Section
    writeln!(f, "\n{}\n", "Victim Transactions".bright_yellow().underline())?;
    let mut idx = 0;
    for (i, tx_hashes) in jit_sandwich_data.victim_swaps_tx_hashes.iter().enumerate() {
        writeln!(f, " {}:",format!("Victims of Frontrun Tx {}", i + 1).yellow())?;
        let swaps = &jit_sandwich_data.victim_swaps[idx];
        let gas_details = &jit_sandwich_data.victim_swaps_gas_details[idx];

        for tx_hash in tx_hashes.iter() {
            writeln!(
                f,
                " - {}: {}",
                format!("Victim Transaction {}", i + 1).bright_magenta(),
                format_etherscan_url(tx_hash)
            )?;

            writeln!(f, "     - {}:", "Actions".bright_blue())?;
            for (j, swap) in swaps.iter().enumerate() {
                writeln!(f, "      {}: {}", format!(" - {}", j + 1).green(), swap)?;

            }

            writeln!(f, "     - {}:", "Gas Details".bright_blue())?;
            gas_details.pretty_print_with_spaces(f, 8)?;
        }

        idx += 1;
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

    let no_of_swaps: usize = jit_sandwich_data.backrun_swaps.len() + 1;
    for (i, burn) in jit_sandwich_data.backrun_burns.iter().enumerate() {
        writeln!(f, "      {}: {}", format!(" - {}", i + no_of_swaps).green(), burn)?;
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

    Ok(())
}

pub fn display_atomic_backrun(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"
          ___  _                  _         ___       _     
         / _ \| |                (_)       / _ \     | |    
        / /_\ \ |_ ___  _ __ ___  _  ___  / /_\ \_ __| |__  
        |  _  | __/ _ \| '_ ` _ \| |/ __| |  _  | '__| '_ \ 
        | | | | || (_) | | | | | | | (__  | | | | |  | |_) |
        \_| |_/\__\___/|_| |_| |_|_|\___| \_| |_/_|  |_.__/ 
    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let atomic_backrun_data = match &bundle.data {
        BundleData::AtomicArb(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // Tx details
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().red())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;
    writeln!(f, "   - Mev Contract: {}", bundle.header.mev_contract)?;

    let tx_url = format!("https://etherscan.io/tx/{:?}", bundle.header.tx_hash).underline();
    writeln!(f, "   - Etherscan: {}", tx_url)?;

    // Backrun Section
    writeln!(f, "\n{}", "Atomic Backrun\n".bright_yellow().underline())?;
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
    writeln!(f, "\n{}", bundle.header.token_profits)?;

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

    // Liquidation Transaction Section
    writeln!(f, "{}\n", "Liquidation Transaction".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Tx Hash".bright_blue(), liquidation_data.liquidation_tx_hash)?;
    writeln!(f, " - {}: {}", "Trigger".bright_blue(), liquidation_data.trigger)?;

    // Swaps Section
    writeln!(f, "\n{}\n", "Liquidation Swaps".bright_yellow().underline())?;
    for (i, swap) in liquidation_data.liquidation_swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }

    // Liquidations Section
    writeln!(f, "\n{}\n", "Liquidations".bright_yellow().underline())?;
    for (i, liquidation) in liquidation_data.liquidations.iter().enumerate() {
        writeln!(f, " - {}: {}", format!("Liquidation {}", i + 1).bright_blue(), liquidation)?;
    }

    // Gas Details Section
    writeln!(f, "\n - {}:", "Gas Details:".bright_blue())?;
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

    writeln!(f, "{}", bundle.header.token_profits)?;
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

    // Frontrun Section
    writeln!(f, "{}\n", "Frontrun Mints".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Mint Tx Hash".bright_blue(), jit_data.frontrun_mint_tx_hash)?;
    writeln!(f, " - {}", "Mints:".bright_blue())?;
    for (i, mint) in jit_data.frontrun_mints.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), mint)?;
    }
    writeln!(f, " - {}:", "Gas Details".bright_blue())?;
    jit_data
        .frontrun_mint_gas_details
        .pretty_print_with_spaces(f, 8)?;

    // Victim Section
    writeln!(f, "\n{}\n", "Victim Swaps".bright_yellow().underline())?;
    for (i, (swaps, gas_details)) in jit_data
        .victim_swaps
        .iter()
        .zip(jit_data.victim_swaps_gas_details.iter())
        .enumerate()
    {
        writeln!(f, " - {}: ", format!("Transaction {}", i + 1).bright_blue())?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "    {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }
        writeln!(f, "   - {}:", "Gas Details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 8)?;
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Burns".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Burn Tx Hash".bright_blue(), jit_data.backrun_burn_tx_hash)?;
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

    Ok(())
}

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

    // Frontrun Section
    writeln!(f, "{}\n", "Frontrun Transactions".bright_yellow().underline())?;
    for (i, ((tx_hash, swaps), gas_details)) in sandwich_data
        .frontrun_tx_hash
        .iter()
        .zip(sandwich_data.frontrun_swaps.iter())
        .zip(sandwich_data.frontrun_gas_details.iter())
        .enumerate()
    {
        writeln!(f, " - {}: {}", format!("Transaction {}", i + 1).bright_blue(), tx_hash)?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "    {}: {}", format!(" - {}", j + 1).green(), swap)?;
        }
        writeln!(f, " - {}:", "Gas Details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 8)?;
    }

    // Victim Section
    writeln!(f, "\n{}\n", "Victim Transactions".bright_yellow().underline())?;
    for ((tx_hashes, swaps), gas_details) in sandwich_data
        .victim_swaps_tx_hashes
        .iter()
        .zip(sandwich_data.victim_swaps.iter())
        .zip(sandwich_data.victim_swaps_gas_details.iter())
    {
        for (j, tx_hash) in tx_hashes.iter().enumerate() {
            writeln!(f, " - {}: {}", format!("Transaction {}", j + 1).bright_blue(), tx_hash)?;
        }
        for (i, swap) in swaps.iter().enumerate() {
            writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
        }
        writeln!(f, " - {}:", "Gas Details".bright_blue())?;
        gas_details.pretty_print_with_spaces(f, 8)?;
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Transaction".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Tx Hash".bright_blue(), sandwich_data.backrun_tx_hash)?;
    for (i, swap) in sandwich_data.backrun_swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
    }
    writeln!(f, " - {}:", "Gas Details".bright_blue())?;
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
    writeln!(f, "{}: \n", "Transaction Details".bold().underline().bright_yellow())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;
    writeln!(f, "   - Mev Contract: {}", bundle.header.mev_contract)?;

    let tx_url = format!("https://etherscan.io/tx/{:?}", bundle.header.tx_hash).underline();
    writeln!(f, "   - Etherscan: {}", tx_url)?;

    // Mev section
    writeln!(f, "\n{}", "MEV:\n".bold().underline().bright_yellow())?;
    writeln!(f, "   - Bundle Profit (USD): {}", format_profit(bundle.header.profit_usd))?;
    writeln!(f, "   - Bribe (USD): {}", (format_bribe(bundle.header.bribe_usd)).to_string().red())?;

    // Cex-dex specific details
    writeln!(f, "\n{}", "Cex-Dex Details:\n".bold().purple().underline())?;
    writeln!(f, "PnL: {}", cex_dex_data.pnl)?;

    for (i, swap) in cex_dex_data.swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!(" - {}", i + 1).green(), swap)?;
        if let Some(stat_arb_detail) = cex_dex_data.stat_arb_details.get(i) {
            writeln!(f, "{}", stat_arb_detail)?;
        } else {
            writeln!(f, "   No arbitrage details found for this swap.")?;
        }
    }

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

fn format_etherscan_url(tx_hash: &FixedBytes<32>) -> String {
    format!("https://etherscan.io/tx/{:?}", tx_hash)
        .underline()
        .to_string()
}
