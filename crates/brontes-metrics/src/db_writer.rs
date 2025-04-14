use std::time::{Duration, Instant};

use prometheus::{Histogram, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec};
use reth_storage_errors::db::DatabaseError;

#[derive(Clone)]
pub struct LibmdbxWriterMetrics {
    // Number of initialized blocks for each tables
    initialized_blocks: IntGaugeVec,
    // Total message latency from receipt to end of write operation
    commit_latency: HistogramVec,
    // Write latency for a single-record write
    write_latency: HistogramVec,
    // Write latency for each batch
    write_latency_batch: Histogram,
    // Write errors per table by error type
    write_errors: IntCounterVec,
    write_error_types: IntCounterVec,
    // Current size of the write queue
    queue_size: IntGauge,
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

        let commit_latency = prometheus::register_histogram_vec!(
            "libmdbx_commit_latency_ms",
            "Time taken from receiving data to completing the write operation",
            &["msg_type"],
            prometheus::exponential_buckets(0.001, 2.0, 25).unwrap()
        )
        .unwrap();

        let write_latency = prometheus::register_histogram_vec!(
            "libmdbx_write_latency_ms",
            "Latency of a single-element write operation",
            &["table"],
            prometheus::exponential_buckets(0.001, 2.0, 25).unwrap()
        )
        .unwrap();

        let write_latency_batch = prometheus::register_histogram!(
            "libmdbx_write_latency_batch_ms",
            "Latency of a batch write operation",
            prometheus::exponential_buckets(0.001, 2.0, 25).unwrap()
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

        let queue_size = prometheus::register_int_gauge!(
            "libmdbx_queue_size",
            "Current size of the write queue"
        )
        .unwrap();

        Self {
            initialized_blocks,
            commit_latency,
            write_latency,
            write_latency_batch,
            write_errors,
            write_error_types,
            queue_size,
        }
    }

    pub fn increment_initialized_blocks(&self, table: &str, count: i64) {
        self.initialized_blocks
            .with_label_values(&[table])
            .add(count);
    }

    /// Instruments the total commit latency, representing the time from the
    /// message's insertion into the queue to the conclusion of the write
    /// operation.  Accepts a string representing the message time, `start_time`
    /// representing the message's insertion time into the queue, and an
    /// optional `end_time`.  If `None`, `end_time` will be set to
    /// `Instant::now()` otherwise the caller can provide an `Instant` to be
    /// used as the end tiem for this observation.
    pub fn observe_commit_latency(
        &self,
        msg_type: &str,
        start_time: Instant,
        end_time: Option<Instant>,
    ) {
        let final_time = end_time.unwrap_or_else(Instant::now);
        let t_total = final_time - start_time;
        self.commit_latency
            .with_label_values(&[msg_type])
            .observe(t_total.as_secs_f64() * 1000_f64);
    }

    /// Instruments the latency of a single database write operation, tagged
    /// with the table name being written to.
    pub fn observe_write_latency(&self, table: &str, duration: Duration) {
        self.write_latency
            .with_label_values(&[table])
            .observe(duration.as_secs_f64() * 1000_f64);
    }

    /// Instruments the latency of a batch write operation.  Since we don't know
    /// what might be in the batch, we'll just have this be a plain
    /// histogram which takes a duration on its own with no tags.
    pub fn observe_write_latency_batch(&self, duration: Duration) {
        self.write_latency_batch
            .observe(duration.as_secs_f64() * 1000_f64);
    }

    /// Instruments the count of errors encountered while writing to the
    /// database, tagged by the type of error encountered
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
            DatabaseError::Other(_) => "Other",
        };

        self.write_error_types
            .with_label_values(&[table, error_type])
            .inc();
    }

    /// Instruments the current size of the write queue.  Queue size will be
    /// bracketed to the max i64 value if it exceeds this but at that point
    /// we probably have much bigger problems
    pub fn set_queue_size(&self, size: usize) {
        let s = size.try_into().unwrap_or(i64::MAX);
        self.queue_size.set(s);
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

    pub fn observe_commit_latency(
        &self,
        msg_type: &str,
        start_time: Instant,
        end_time: Option<Instant>,
    ) {
        if let Some(metrics) = &self.0 {
            metrics.observe_commit_latency(msg_type, start_time, end_time);
        }
    }

    pub fn observe_write_latency(&self, table: &str, duration: Duration) {
        if let Some(metrics) = &self.0 {
            metrics.observe_write_latency(table, duration);
        }
    }

    pub fn observe_write_latency_batch(&self, duration: Duration) {
        if let Some(metrics) = &self.0 {
            metrics.observe_write_latency_batch(duration);
        }
    }

    pub fn increment_write_errors(&self, table: &str, error: &DatabaseError) {
        if let Some(metrics) = &self.0 {
            metrics.increment_write_errors(table, error);
        }
    }

    pub fn set_queue_size(&self, size: usize) {
        if let Some(metrics) = &self.0 {
            metrics.set_queue_size(size);
        }
    }
}
