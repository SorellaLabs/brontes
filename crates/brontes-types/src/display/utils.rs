use std::fmt;

use colored::{ColoredString, Colorize};
use indoc::indoc;

use crate::mev::{Bundle, BundleData, MevType};

pub fn print_mev_type_header(mev_type: MevType, f: &mut fmt::Formatter) -> fmt::Result {
    match mev_type {
        MevType::Sandwich => {
            let ascii_header = indoc! {r#"
         _____                 _          _      _     
        /  ___|               | |        (_)    | |    
        \ `--.  __ _ _ __   __| |_      ___  ___| |__  
         `--. \/ _` | '_ \ / _` \ \ /\ / / |/ __| '_ \ 
        /\__/ / (_| | | | | (_| |\ V  V /| | (__| | | |
        \____/ \__,_|_| |_|\__,_| \_/\_/ |_|\___|_| |_|
        ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.bright_red())?;
            }
        }
        MevType::Backrun => {
            let ascii_header = indoc! {r#"
            ______            _                     
            | ___ \          | |                    
            | |_/ / __ _  ___| | ___ __ _   _ _ __  
            | ___ \/ _` |/ __| |/ / '__| | | | '_ \ 
            | |_/ / (_| | (__|   <| |  | |_| | | | |
            \____/ \__,_|\___|_|\_\_|   \__,_|_| |_|
        ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.green())?;
            }
        }
        MevType::CexDex => {
            let ascii_header = indoc! {r#"
             _____ _        _           ___       _     
            /  ___| |      | |         / _ \     | |    
            \ `--.| |_ __ _| |_ ______/ /_\ \_ __| |__  
             `--. \ __/ _` | __|______|  _  | '__| '_ \ 
            /\__/ / || (_| | |_       | | | | |  | |_) |
            \____/ \__\__,_|\__|      \_| |_/_|  |_.__/ 
                                                        
        ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.purple())?;
            }
        }

        MevType::Jit => {
            let ascii_header = indoc! {r#"
             ___ _ _          _     _             _     _ _ _         
            |_  (_) |        | |   (_)           (_)   | (_) |        
              | |_| |_ ______| |    _  __ _ _   _ _  __| |_| |_ _   _ 
              | | | __|______| |   | |/ _` | | | | |/ _` | | __| | | |
          /\__/ / | |_       | |___| | (_| | |_| | | (_| | | |_| |_| |
          \____/|_|\__|      \_____/_|\__, |\__,_|_|\__,_|_|\__|\__, |
                                         | |                     __/ |
                                         |_|                    |___/ 
        ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.blue())?;
            }
        }

        MevType::Liquidation => {
            let ascii_header = indoc! {r#"
             _     _             _     _       _   _             
            | |   (_)           (_)   | |     | | (_)            
            | |    _  __ _ _   _ _  __| | __ _| |_ _  ___  _ __  
            | |   | |/ _` | | | | |/ _` |/ _` | __| |/ _ \| '_ \ 
            | |___| | (_| | |_| | | (_| | (_| | |_| | (_) | | | |
            \_____/_|\__, |\__,_|_|\__,_|\__,_|\__|_|\___/|_| |_|
                        | |                                      
                        |_|                                      
        ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.cyan())?;
            }
        }
        MevType::JitSandwich => {
            let ascii_header = indoc! {r#"
             ___ _ _          _____                 _          _      _     
            |_  (_) |        /  ___|               | |        (_)    | |    
              | |_| |_ ______\ `--.  __ _ _ __   __| |_      ___  ___| |__  
              | | | __|______|`--. \/ _` | '_ \ / _` \ \ /\ / / |/ __| '_ \ 
          /\__/ / | |_       /\__/ / (_| | | | | (_| |\ V  V /| | (__| | | |
          \____/|_|\__|      \____/ \__,_|_| |_|\__,_| \_/\_/ |_|\___|_| |_|                                                                                            
       ""#};

            for line in ascii_header.lines() {
                writeln!(f, "{}", line.magenta())?;
            }
        }
        _ => (),
    };

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
        writeln!(f, " - {}: {}", format!("Transaction {}", i + 1).bright_blue(), tx_hash)?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "   {}: {}", format!("Swap {}", j + 1).green(), swap)?;
        }
        if let Some(mint_list) = mints {
            for (j, mint) in mint_list.iter().enumerate() {
                writeln!(f, "   {}: {:?}", format!("Mint {}", j + 1).green(), mint)?;
            }
        }
        writeln!(f, " - {}: {}", "Gas Details".bright_blue(), gas_details)?;
    }

    // Victim Section
    writeln!(f, "\n{}\n", "Victim Transactions".bright_yellow().underline())?;
    for (i, ((tx_hashes, swaps), gas_details)) in jit_sandwich_data
        .victim_swaps_tx_hashes
        .iter()
        .zip(jit_sandwich_data.victim_swaps.iter())
        .zip(jit_sandwich_data.victim_swaps_gas_details.iter())
        .enumerate()
    {
        for (j, tx_hash) in tx_hashes.iter().enumerate() {
            writeln!(f, " - {}: {}", format!("Transaction {}", j + 1).bright_blue(), tx_hash)?;
        }
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "   {}: {}", format!("Swap {}", j + 1).green(), swap)?;
        }
        writeln!(f, " - {}: {}", "Gas Details".bright_blue(), gas_details)?;
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Transaction".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Tx Hash".bright_blue(), jit_sandwich_data.backrun_tx_hash)?;
    for (i, swap) in jit_sandwich_data.backrun_swaps.iter().enumerate() {
        writeln!(f, "   {}: {}", format!("Swap {}", i + 1).green(), swap)?;
    }
    for (i, burn) in jit_sandwich_data.backrun_burns.iter().enumerate() {
        writeln!(f, "   {}: {:?}", format!("Burn {}", i + 1).green(), burn)?;
    }
    writeln!(f, " - {}: {}", "Gas Details".bright_blue(), jit_sandwich_data.backrun_gas_details)?;

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
        format_bribe(bundle.header.bribe_usd).to_string().bright_red()
    )?;

    Ok(())
}

pub fn display_atomic_backrun(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
    let ascii_header = indoc! {r#"
        ______            _                     
        | ___ \          | |                    
        | |_/ / __ _  ___| | ___ __ _   _ _ __  
        | ___ \/ _` |/ __| |/ / '__| | | | '_ \ 
        | |_/ / (_| | (__|   <| |  | |_| | | | |
        \____/ \__,_|\___|_|\_\_|   \__,_|_| |_|

    "#};

    for line in ascii_header.lines() {
        writeln!(f, "{}", line.bright_red())?;
    }

    let atomic_backrun_data = match &bundle.data {
        BundleData::AtomicBackrun(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // Backrun Section
    writeln!(f, "{}", "Atomic Backrun\n".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Transaction Hash".bright_blue(), atomic_backrun_data.tx_hash)?;
    writeln!(f, " - {}", "Swaps:".bright_blue())?;
    for (i, swap) in atomic_backrun_data.swaps.iter().enumerate() {
        writeln!(f, "    {}: {}", format!("Swap {}", i + 1).green(), swap)?;
    }
    writeln!(f, " - {}: {}", "Gas Details".bright_blue(), atomic_backrun_data.gas_details)?;

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
        format_bribe(bundle.header.bribe_usd).to_string().bright_red()
    )?;

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
        writeln!(f, " - {}: {}", format!("Swap {}", i + 1).bright_blue(), swap)?;
    }

    // Liquidations Section
    writeln!(f, "\n{}\n", "Liquidations".bright_yellow().underline())?;
    for (i, liquidation) in liquidation_data.liquidations.iter().enumerate() {
        writeln!(f, " - {}: {:?}", format!("Liquidation {}", i + 1).bright_blue(), liquidation)?;
    }

    // Gas Details Section
    writeln!(f, "\n{}\n", "Gas Details".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Details".bright_blue(), liquidation_data.gas_details)?;

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
        format_bribe(bundle.header.bribe_usd).to_string().bright_red()
    )?;
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
        writeln!(f, "    {}: {:?}", format!("Mint {}", i + 1).green(), mint)?;
    }
    writeln!(f, " - {}: {}", "Gas Details".bright_blue(), jit_data.frontrun_mint_gas_details)?;

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
            writeln!(f, "    {}: {}", format!("Swap {}", j + 1).green(), swap)?;
        }
        writeln!(f, "   - {}: {}", "Gas Details".bright_blue(), gas_details)?;
    }

    // Backrun Section
    writeln!(f, "\n{}\n", "Backrun Burns".bright_yellow().underline())?;
    writeln!(f, " - {}: {}", "Burn Tx Hash".bright_blue(), jit_data.backrun_burn_tx_hash)?;
    writeln!(f, " - {}", "Burns:".bright_blue())?;
    for (i, burn) in jit_data.backrun_burns.iter().enumerate() {
        writeln!(f, "    {}: {:?}", format!("Burn {}", i + 1).green(), burn)?;
    }
    writeln!(f, " - {}: {}", "Gas Details".bright_blue(), jit_data.backrun_burn_gas_details)?;

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
        format_bribe(bundle.header.bribe_usd).to_string().bright_red()
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

    // Iterate over the frontrun transactions
    for (i, ((tx_hash, swaps), gas_details)) in sandwich_data
        .frontrun_tx_hash
        .iter()
        .zip(sandwich_data.frontrun_swaps.iter())
        .zip(sandwich_data.frontrun_gas_details.iter())
        .enumerate()
    {
        writeln!(f, "{} {}: ", "Frontrun".bold().red(), i + 1)?;
        writeln!(f, "Transaction hash: {}", tx_hash)?;
        writeln!(f, "Swaps:")?;
        for (j, swap) in swaps.iter().enumerate() {
            writeln!(f, "  Swap {}: {}", j + 1, swap)?;
        }
        writeln!(f, "Gas details: {}", gas_details)?;

        // Process corresponding victim transactions for this frontrun
        if let Some(victim_tx_hashes) = sandwich_data.victim_swaps_tx_hashes.get(i) {
            // Create an iterator that zips the victim transaction hashes with corresponding
            // swaps
            let victims_iter = victim_tx_hashes
                .iter()
                .zip(sandwich_data.victim_swaps.iter());

            for (victim_tx_hash, victim_swaps) in victims_iter {
                writeln!(f, "Victim Transaction: {}", victim_tx_hash)?;
                for (k, swap) in victim_swaps.iter().enumerate() {
                    writeln!(f, "  Swap {}: {}", k + 1, swap)?;
                }
            }
        }
    }

    // Process the backrun transaction
    writeln!(f, "Backrun: ")?;
    writeln!(f, "Transaction hash: {}", sandwich_data.backrun_tx_hash)?;
    writeln!(f, "Swaps:")?;
    for (j, swap) in sandwich_data.backrun_swaps.iter().enumerate() {
        writeln!(f, "  Swap {}: {}", j + 1, swap)?;
    }
    writeln!(f, "Gas details: {}", sandwich_data.backrun_gas_details)?;
    writeln!(f, "   - Bundle Profit (USD): {}", format_profit(bundle.header.profit_usd))?;
    writeln!(f, "   - Bribe (USD): {}", (format_bribe(bundle.header.bribe_usd)).to_string().red())?;

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
    writeln!(f, "{}: ", "Transaction Details".bold().underline().red())?;
    writeln!(f, "   - Tx Index: {}", bundle.header.tx_index.to_string().bold())?;
    writeln!(f, "   - EOA: {}", bundle.header.eoa)?;
    writeln!(f, "   - Mev Contract: {}", bundle.header.mev_contract)?;

    let tx_url = format!("https://etherscan.io/tx/{:?}", bundle.header.tx_hash).underline();
    writeln!(f, "   - Etherscan: {}", tx_url)?;

    // Mev section
    writeln!(f, "\n{}", "Mev:".bold().red().underline())?;
    writeln!(f, "   - Bundle Profit (USD): {}", format_profit(bundle.header.profit_usd))?;
    writeln!(f, "   - Bribe (USD): {}", (format_bribe(bundle.header.bribe_usd)).to_string().red())?;

    // Cex-dex specific details
    writeln!(f, "\n{}", "Cex-Dex Details:".bold().purple().underline())?;
    for swap in cex_dex_data.swaps.iter() {
        writeln!(f, "   - Swap: {}", swap)?;
        /*writeln!(
            f,
            "   - Cex-Dex Delta: {}",
            swap.amount_in * (cex_dex_data.prices_price[idx + 1] - cex_dex_data.prices_price[idx])
        )?;  */
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
