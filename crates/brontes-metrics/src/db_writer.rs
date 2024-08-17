use std::time::{Duration, Instant};

use prometheus::{HistogramVec, IntCounterVec, IntGaugeVec};
use reth_interfaces::db::DatabaseError;

#[derive(Clone)]
pub struct LibmdbxWriterMetrics {
    // Number of initialized blocks for each tables
    initialized_blocks:  IntGaugeVec,
    // Write latency for each table
    write_latency:       HistogramVec,
    // Write latency for each batch
    write_latency_batch: HistogramVec,
    // Write errors per table by error type
    write_errors:        IntCounterVec,
    write_error_types:   IntCounterVec,
    // Current size of the write queue for each table
    queue_size:          IntGaugeVec,
}

impl Default for LibmdbxWriterMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LibmdbxWriterMetrics {
    pub fn new() -> Self {
        let initialized_blocks = prometheus::register_int_gauge_vec!(
            "libmdbx_initialized_blocks",
            "Number of initialized blocks for each table",
            &["table"]
        )
        .unwrap();

        let write_latency = prometheus::register_histogram_vec!(
            "libmdbx_write_latency_seconds",
            "Time taken from receiving data to completing the write operation",
            &["table"],
            prometheus::exponential_buckets(0.001, 2.0, 20).unwrap()
        )
        .unwrap();

        let write_errors = prometheus::register_int_counter_vec!(
            "libmdbx_write_errors",
            "Number of write errors for each table",
            &["table"]
        )
        .unwrap();

        let write_error_types = prometheus::register_int_counter_vec!(
            "libmdbx_write_error_types",
            "Types of errors encountered during write operations",
            &["table", "error_type"]
        )
        .unwrap();

        let queue_size = prometheus::register_int_gauge_vec!(
            "libmdbx_queue_size",
            "Current size of the write queue for each table",
            &["table"]
        )
        .unwrap();

        Self { initialized_blocks, write_latency, write_errors, write_error_types, queue_size }
    }

    pub fn increment_initialized_blocks(&self, table: &str, count: i64) {
        self.initialized_blocks
            .with_label_values(&[table])
            .add(count);
    }

    pub fn observe_write_latency(&self, table: &str, duration: Duration) {
        self.write_latency
            .with_label_values(&[table])
            .observe(duration.as_secs_f64());
    }

    pub fn increment_write_errors(&self, table: &str, error: &DatabaseError) {
        self.write_errors.with_label_values(&[table]).inc();

        let error_type = match error {
            DatabaseError::Open(_) => "Open",
            DatabaseError::CreateTable(_) => "CreateTable",
            DatabaseError::Write(_) => "Write",
            DatabaseError::Read(_) => "Read",
            DatabaseError::Delete(_) => "Delete",
            DatabaseError::Commit(_) => "Commit",
            DatabaseError::InitTx(_) => "InitTx",
            DatabaseError::InitCursor(_) => "InitCursor",
            DatabaseError::Decode => "Decode",
            DatabaseError::Stats(_) => "Stats",
            DatabaseError::LogLevelUnavailable(_) => "LogLevelUnavailable",
        };

        self.write_error_types
            .with_label_values(&[table, error_type])
            .inc();
    }

    pub fn update_queue_size(&self, table: &str, size: i64) {
        self.queue_size.with_label_values(&[table]).set(size);
    }
}

#[derive(Clone)]
pub struct WriterMetrics(Option<LibmdbxWriterMetrics>);

impl WriterMetrics {
    pub fn new(metrics: bool) -> Self {
        if metrics {
            Self(Some(LibmdbxWriterMetrics::new()))
        } else {
            Self(None)
        }
    }

    pub fn increment_initialized_blocks(&self, table: &str, count: i64) {
        if let Some(metrics) = &self.0 {
            metrics.increment_initialized_blocks(table, count);
        }
    }

    pub fn observe_write_latency(&self, table: &str, duration: Duration) {
        if let Some(metrics) = &self.0 {
            metrics.observe_write_latency(table, duration);
        }
    }

    pub fn increment_write_errors(&self, table: &str, error: &DatabaseError) {
        if let Some(metrics) = &self.0 {
            metrics.increment_write_errors(table, error);
        }
    }

    pub fn update_queue_size(&self, table: &str, size: i64) {
        if let Some(metrics) = &self.0 {
            metrics.update_queue_size(table, size);
        }
    }
}
