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
        for (function_name, calls) in &self.0 {
            let call_amount = calls.len();
            let total_time_ms: u128 = calls.iter().map(|call| call.as_millis()).sum();

            let average_duration_ms = total_time_ms / call_amount as u128;
            let bottom_q = &calls[(call_amount as f64 * 0.25) as usize].as_millis();
            let top_q = &calls[(call_amount as f64 * 0.75) as usize].as_millis();

            tracing::info!(target: "brontes::call_details",
                "\n------------ {function_name} call benches \
                 -----------\ntotal_calls={call_amount}\ntotal_time={total_time_ms}ms\\
                 naverage_call_duration={average_duration_ms}ms\n25th_pct_time={bottom_q}ms\\
                 n75th_pct_time={top_q}ms"
            )
        }
    }
}
