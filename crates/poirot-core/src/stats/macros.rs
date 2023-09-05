// TODO: All of these can just be functions and then have them tagged with #[inline(always)]
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
    ($tx:expr, $err:expr) => {
        let tx_hash = format!("{:#x}", $tx);
        let result = format!("Error Parsing Trace").bright_red();
        let error = format!("{:?}", $err).bright_red();
        let mut values_str = format!("{}, Tx Hash = {}, Error = {}", result, tx_hash, error);
        /*
        for (key, val) in $values.iter() {
            values_str = format!("{}, {} = {}", values_str, key, val);
        } */
        // replace `println!` with your logging mechanism
        info!("result = {}", values_str);
    };
}

#[macro_export]
macro_rules! success_trace {
    ($tx:expr) => {
        let tx_hash = format!("{:#x}", $tx);
        let result = format!("Successfully Parsed Trace").bright_green();
        let mut values_str = format!("{}, Tx Hash = {}", result, tx_hash);

        info!("result = {}", values_str);
    };
}

#[macro_export]
macro_rules! init_tx {
    ($tx:expr, $idx:expr, $total_len:expr) => {
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
    ($blk:expr, $tx:expr) => {{
        let tx_hash = format!("{:#x}", $tx);
        info!("result = \"Successfully Parsed Transaction\", tx_hash = {}\n", tx_hash);
    }};
}

#[macro_export]
macro_rules! init_block {
    ($blk:expr, $start_blk:expr, $end_blk:expr) => {{
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
