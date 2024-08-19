use std::{sync::OnceLock, time::Instant};

use clickhouse::{error::Error, query};
use db_interfaces::{clickhouse::errors::ClickhouseError, errors::DatabaseError};
use eyre::Report;
use prometheus::{CounterVec, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec};

fn query_speed() -> &'static HistogramVec {
    static QUERY_SPEED: OnceLock<HistogramVec> = OnceLock::new();
    QUERY_SPEED.get_or_init(|| {
        prometheus::register_histogram_vec!(
            "initialization_query_speed_us",
            "Time taken for each query during initialization in microseconds",
            &["table", "block_count"]
        )
        .unwrap()
    })
}

fn query_errors() -> &'static IntCounterVec {
    static QUERY_ERRORS: OnceLock<IntCounterVec> = OnceLock::new();
    QUERY_ERRORS.get_or_init(|| {
        prometheus::register_int_counter_vec!(
            "initialization_query_errors",
            "Number of query errors for each table",
            &["table"]
        )
        .unwrap()
    })
}

fn query_error_types() -> &'static IntCounterVec {
    static QUERY_SPEED: OnceLock<IntCounterVec> = OnceLock::new();
    QUERY_SPEED.get_or_init(|| {
        prometheus::register_int_counter_vec!(
            "initialization_query_error_types",
            "Types of errors encountered during initialization queries",
            &["table", "error_type"]
        )
        .unwrap()
    })
}
#[derive(Clone)]
pub struct InitializationMetrics {
    query_speed:       &'static HistogramVec,
    query_errors:      &'static IntCounterVec,
    query_error_types: &'static IntCounterVec,
}

impl Default for InitializationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl InitializationMetrics {
    pub fn new() -> Self {
        let buckets = prometheus::exponential_buckets(1.0, 2.0, 22).unwrap();
        let query_speed = query_speed();
        let query_errors = query_errors();
        let query_error_types = query_error_types();

        Self { query_speed, query_errors, query_error_types }
    }

    pub fn measure_query<R>(&self, table: &str, block_count: u64, f: impl FnOnce() -> R) -> R {
        let now = Instant::now();
        let res = f();
        let elapsed = now.elapsed().as_micros();
        self.query_speed
            .with_label_values(&[table, &block_count.to_string()])
            .observe(elapsed as f64);
        res
    }

    pub fn increment_query_errors(&self, table: &str, error: &Report) {
        self.query_errors.with_label_values(&[table]).inc();

        let error_type = self.categorize_error(error);
        self.query_error_types
            .with_label_values(&[table, &error_type])
            .inc();
    }

    fn categorize_error(&self, error: &Report) -> String {
        if let Some(db_error) = error.downcast_ref::<DatabaseError>() {
            match db_error {
                DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(native_error)) => {
                    match native_error {
                        Error::InvalidParams(_) => "InvalidParams",
                        Error::Network(_) => "Network",
                        Error::Compression(_) => "Compression",
                        Error::Decompression(_) => "Decompression",
                        Error::RowNotFound => "RowNotFound",
                        Error::SequenceMustHaveLength => "SequenceMustHaveLength",
                        Error::DeserializeAnyNotSupported => "DeserializeAnyNotSupported",
                        Error::NotEnoughData => "NotEnoughData",
                        Error::InvalidUtf8Encoding(_) => "InvalidUtf8Encoding",
                        Error::InvalidTagEncoding(_) => "InvalidTagEncoding",
                        Error::Custom(_) => "Custom",
                        Error::BadResponse(_) => "BadResponse",
                        Error::TimedOut => "TimedOut",
                        Error::TooSmallBuffer(_) => "TooSmallBuffer",
                        _ => "OtherClickhouseNative",
                    }
                    .to_string()
                }
                DatabaseError::ClickhouseError(ClickhouseError::SqlFileReadError(_)) => {
                    "SqlFileReadError".to_string()
                }
                _ => "OtherDatabaseError".to_string(),
            }
        } else if error.to_string().contains("no block times found") {
            "EmptyBlockTimes".to_string()
        } else {
            "OtherError".to_string()
        }
    }
}

#[derive(Clone)]
pub struct InitMetrics(Option<InitializationMetrics>);

impl InitMetrics {
    pub fn new(metrics: bool) -> Self {
        if metrics {
            Self(Some(InitializationMetrics::new()))
        } else {
            Self(None)
        }
    }

    pub fn measure_query<R>(&self, table: &str, block_count: u64, f: impl FnOnce() -> R) -> R {
        if let Some(metrics) = &self.0 {
            metrics.measure_query(table, block_count, f)
        } else {
            f()
        }
    }

    pub fn increment_query_errors(&self, table: &str, error: &Report) {
        if let Some(metrics) = &self.0 {
            metrics.increment_query_errors(table, error);
        }
    }
}
