use crate::stats::{display::ErrorStats, format_color};
use colored::{Color, Colorize};
use revm_primitives::B256;
pub struct ParserStatsLayer;



pub struct BlockStats {
    pub block_num: u64,
    pub tx_stats: Vec<TransactionStats>,
}

/// blocl level stats
impl BlockStats {
    pub fn display_stats(&self) {
        println!(
            "{}",
            format_color("STATS FOR BLOCK", self.block_num as usize, Color::BrightBlue).bold()
        );
        println!("----------------------------------------------------------------------------------------");
        println!("{}", format_color("Total Transactions", self.tx_stats.len(), Color::Blue));
        println!(
            "{}",
            format_color(
                "Total Traces",
                self.tx_stats
                    .iter()
                    .map(|tx| tx.error_parses.len() + tx.successful_parses)
                    .sum::<usize>(),
                Color::Blue
            )
        );
        println!(
            "{}",
            format_color(
                "Successful Parses",
                self.tx_stats.iter().map(|tx| tx.successful_parses).sum::<usize>(),
                Color::Blue
            )
        );
        println!(
            "{}",
            format_color(
                "Total Errors",
                self.tx_stats.iter().map(|tx| tx.error_parses.len()).sum::<usize>(),
                Color::Blue
            )
        );

        let mut errors = ErrorStats::default();
        for err in self.tx_stats.iter().flat_map(|tx| &tx.error_parses) {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Blue, "");
        println!();
    }
}


/// tx level stats
pub struct TransactionStats {
    pub tx_hash: B256,
    pub successful_parses: usize,
    pub error_parses: Vec<TraceStat>,
}

impl TransactionStats {
    pub fn display_stats(&self) {
        let spacing = " ".repeat(8);

        println!(
            "{}{}",
            spacing,
            format_color(
                "STATS FOR TRANSACTION",
                format!("{:#x}", self.tx_hash),
                Color::BrightCyan
            )
            .bold()
        );
        println!("{}----------------------------------------------------------------------------------------", spacing);
        println!(
            "{}{}",
            spacing,
            format_color(
                "Total Traces",
                self.successful_parses + self.error_parses.len(),
                Color::Cyan
            )
        );
        println!(
            "{}{}",
            spacing,
            format_color("Successful Parses", self.successful_parses, Color::Cyan)
        );
        println!(
            "{}{}",
            spacing,
            format_color("Total Errors", self.error_parses.len(), Color::Cyan)
        );

        let mut errors = ErrorStats::default();
        for err in &self.error_parses {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Cyan, &spacing);

        for trace in &self.error_parses {
            println!(
                "{}{} - {:?}",
                &spacing,
                format_color("Error - Trace", trace.idx, Color::Cyan),
                trace.error
            );
        }
        println!();
    }
}


/// tx level stats 
/// gives the trace idx in the tx + the error
pub struct TraceStat {
    pub idx: usize,
    pub error: Box<dyn std::error::Error + Sync + Send + 'static>,
}
