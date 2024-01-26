use std::fmt;

use colored::Colorize;
use indoc::indoc;

use crate::classified_mev::MevType;

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
