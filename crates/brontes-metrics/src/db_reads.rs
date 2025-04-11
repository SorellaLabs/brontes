use std::time::Instant;

use prometheus::{HistogramVec, IntCounterVec};

#[derive(Clone)]
pub struct LibmdbxMetrics {
    read_speed: HistogramVec,
    read_count: IntCounterVec,
}
impl Default for LibmdbxMetrics {
    fn default() -> Self {
        Self::new()
    }
}
impl LibmdbxMetrics {
    pub fn new() -> Self {
        let buckets = prometheus::exponential_buckets(1.0, 2.0, 22).unwrap();

        let read_speed = prometheus::register_histogram_vec!(
            "libmdbx_read_speed_us",
            "the time for a libmdbx read in US",
            &["function_name"],
            buckets.clone()
        )
        .unwrap();

        let read_count = prometheus::register_int_counter_vec!(
            "libmdbx_read_count",
            "amount of reads a function has done",
            &["function_name"]
        )
        .unwrap();

        Self { read_count, read_speed }
    }

    pub fn db_read<R>(self, fn_name: &str, f: impl FnOnce() -> R) -> R {
        if self
            .read_count
            .get_metric_with_label_values(&[fn_name])
            .is_err()
        {
            panic!("{fn_name:?}");
        }

        self.read_count.with_label_values(&[fn_name]).inc();

        let now = Instant::now();
        let res = f();
        let elasped = now.elapsed().as_micros();

        self.read_speed
            .with_label_values(&[fn_name])
            .observe(elasped as f64);

        res
    }
}
