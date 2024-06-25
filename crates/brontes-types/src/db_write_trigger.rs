use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct DbWriteTrigger {
    switch: Arc<AtomicBool>,
}
