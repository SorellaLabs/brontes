use std::time::Duration;

use brontes_types::FastHashMap;

#[derive(Debug, Default)]
pub struct FunctionCallBench(FastHashMap<String, Vec<Duration>>);

impl FunctionCallBench {
    pub fn add_bench(&mut self, function_name: String, runtime: Duration) {
        self.0.entry(function_name).or_default().push(runtime);
    }
}

impl Drop for FunctionCallBench {
    fn drop(&mut self) {
        for (function_name, calls) in &mut self.0 {
            calls.sort_unstable();

            let call_amount = calls.len();
            let total_time_ms: u128 = calls.iter().map(|call| call.as_millis()).sum();

            if call_amount == 0 {
                continue;
            }

            let average_duration_ms = total_time_ms / call_amount as u128;
            let bottom_q = &calls[(call_amount as f64 * 0.25) as usize].as_millis();
            let top_q = &calls[(call_amount as f64 * 0.75) as usize].as_millis();

            tracing::debug!(target: "brontes::call_details",
                r#"
------------ {function_name} call benches-----------
total_calls={call_amount}
total_time={total_time_ms}ms
average_call_duration={average_duration_ms}ms
25th_pct_time={bottom_q}ms
75th_pct_time={top_q}ms
                "#
            )
        }
    }
}
