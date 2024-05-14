use std::time::Instant;

use prometheus::{HistogramVec, IntCounterVec, IntGaugeVec};

#[derive(Clone)]
pub struct CacheData {
    cache_read_bytes:  IntCounterVec,
    cache_read_speed:  HistogramVec,
    cache_write_bytes: IntCounterVec,
    cache_write_speed: HistogramVec,
}

impl Default for CacheData {
    fn default() -> Self {
        Self::new()
    }
}
impl CacheData {
    pub fn new() -> Self {
        let buckets = prometheus::exponential_buckets(1.0, 2.0, 15).unwrap();

        let read_speed = prometheus::register_histogram_vec!(
            "libmdbx_cache_read_speed",
            "libmdbx cache speed read",
            &["table"],
            buckets.clone()
        )
        .unwrap();

        let write_speed = prometheus::register_histogram_vec!(
            "libmdbx_cache_write_speed",
            "libmdbx cache speed write",
            &["table"],
            buckets.clone()
        )
        .unwrap();

        let read_count = prometheus::register_int_counter_vec!(
            "libmdbx_cache_read_bytes",
            "cache read bytes",
            &["table"]
        )
        .unwrap();

        let write_bytes = prometheus::register_int_counter_vec!(
            "libmdbx_cache_read_bytes",
            "cache read bytes",
            &["table"]
        )
        .unwrap();

        Self {
            cache_write_speed: write_speed,
            cache_read_speed:  read_speed,
            cache_read_bytes:  read_count,
            cache_write_bytes: write_bytes,
        }
    }

    pub fn cache_read_bytes<S>(&self, table: &str, amount: usize) {
        self.cache_read_bytes
            .with_label_values(&[table])
            .inc_by((std::mem::size_of::<S>() * amount) as u64);
    }

    pub fn cache_read<R>(self, table: &str, f: impl FnOnce() -> R) -> R {
        let now = Instant::now();
        let res = f();
        let elasped = now.elapsed().as_nanos();

        self.cache_read_speed
            .with_label_values(&[table])
            .observe(elasped as f64);

        res
    }

    pub fn cache_write<R, S>(self, table: &str, f: impl FnOnce() -> R) -> R {
        self.cache_write_bytes
            .with_label_values(&[table])
            .inc_by(std::mem::size_of::<R>() as u64);

        let now = Instant::now();
        let res = f();
        let elasped = now.elapsed().as_nanos();

        self.cache_write_speed
            .with_label_values(&[table])
            .observe(elasped as f64);

        res
    }
}
