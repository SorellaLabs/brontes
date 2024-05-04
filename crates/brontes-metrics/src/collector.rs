use std::sync::OnceLock;

use tokio::sync::mpsc::UnboundedSender;

pub struct Test1 {}

pub enum BrontesMetricsCollection {}

pub static COLLECTOR: OnceLock<UnboundedSender<BrontesMetricsCollection>> = OnceLock::new();
