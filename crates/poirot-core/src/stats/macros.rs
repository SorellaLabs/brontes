#[macro_export]
macro_rules! init_trace {
    ($tx:expr, $idx:expr, $total:expr) => {{
        let message =
            format!("Starting Trace {}", format!("{}/{}", $idx + 1, $total).bright_cyan());
        info!(message = message);
    }};
}

#[macro_export]
macro_rules! error_trace {
    ($tx:expr, $idx:expr, $err:expr) => {
        {
            let error: Box<dyn std::error::Error + Sync + Send + 'static> = Box::new($err);

            error!(?error);

            let mut tx_stats = TX_STATS.lock().unwrap();
            let tx_stat = tx_stats.get_mut($tx).unwrap();

            tx_stat.error_parses.push(TraceStat { idx: $idx, error });
        }
    };
}

#[macro_export]
macro_rules! success_trace {
    ($tx:expr, $( $key:ident = $val:expr ),* $(,)? ) => {
        {
            let mut tx_stats = TX_STATS.lock().unwrap(); // locks the Mutex
            let tx_stat = tx_stats.get_mut($tx).unwrap();

            tx_stat.successful_parses += 1;

            let tx_hash = format!("{:#x}", $tx);
            info!(result = "Successfully Parsed Trace", tx_hash = tx_hash, $( $key = $val ),*);

        }
    };
}

#[macro_export]
macro_rules! init_tx {
    ($tx:expr, $idx:expr, $total_len:expr) => {
        {
            let mut tx_stats = TX_STATS.lock().unwrap();
            tx_stats.entry($tx).or_insert_with(|| TransactionStats {
                tx_hash: $tx,
                successful_parses: 0,
                error_parses: Vec::new(),
            });
        }

        let tx_hash = format!("{:#x}", $tx);
        let message = format!(
            "{}",
            format!("Starting Transaction Trace {} / {}", $idx + 1, $total_len)
                .bright_blue()
                .bold()
        );
        info!(message = message, tx_hash = tx_hash);
    };
}

#[macro_export]
macro_rules! success_tx {
    ($blk:expr, $tx:expr) => {
        
        {
            let mut block_stats = BLOCK_STATS.lock().unwrap();
            let mut tx_stats = TX_STATS.lock().unwrap();

            let tx_stat = tx_stats.remove(&$tx).unwrap();

            let block_stat = block_stats.get_mut(&$blk).unwrap();
            block_stat.tx_stats.push(tx_stat);

            let tx_hash = format!("{:#x}", $tx);
            info!(result = "Successfully Parsed Transaction", tx_hash = tx_hash);
        }
    };
}

#[macro_export]
macro_rules! init_block {
    ($blk:expr, $start_blk:expr, $end_blk:expr) => {{
        let mut block_stats = poirot_core::BLOCK_STATS.lock().unwrap();
        let block_stat = block_stats.entry($blk).or_insert_with(|| {
            poirot_core::stats::stats::BlockStats { block_num: $blk, tx_stats: Vec::new() }
        });

        let progress = format!(
            "Progress: {} / {}",
            ($blk - $start_blk + 1) as usize,
            ($end_blk - $start_blk) as usize
        )
        .bright_blue()
        .bold();
        let message = format!(
            "Starting Parsing Block {} --- Progress: {}",
            format!("{}", $blk).bright_blue().bold(),
            progress
        );
        info!(message = message);
    }};
}

#[macro_export]
macro_rules! success_block {
    ($blk:expr) => {{
        let message =
            format!("Successfuly Parsed Block {}", format!("{}", $blk).bright_blue().bold());
        info!(message = message);
    }};
}

// displays all the stuff
#[macro_export]
macro_rules! success_all {
    ($start_blk:expr, $end_blk:expr, $verbosity:expr) => {{
        let message = format!(
            "Successfuly Parsed Blocks {}",
            format!("{} to {}", $start_blk, $end_blk).bright_blue().bold()
        );
        info!(message = message);
        poirot_core::stats::display::display_all_stats($verbosity);
    }};
}
