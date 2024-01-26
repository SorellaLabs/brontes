use std::fmt;

use colored::Colorize;
use indoc::indoc;

use crate::classified_mev::{Bundle, BundleData, MevType};

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

pub fn display_sandwich(bundle: &Bundle, f: &mut fmt::Formatter) -> fmt::Result {
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

    let sandwich_data = match &bundle.data {
        BundleData::Sandwich(data) => data,
        _ => panic!("Wrong bundle type"),
    };

    // Iterate over the frontrun transactions
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

    Ok(())
}
