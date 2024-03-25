//! Used to generate a queue for our tests to execute.
//! This is done so that we can limit the amount of cpu usage of the
//! tests to ensure we aren't nuking peoples computers / ci servers

use std::sync::Mutex;

use once_cell::sync::OnceCell;

const MAX_TEST_THREADS: usize = 12;

static RUNNING_INFO: OnceCell<Mutex<(usize, usize)>> = OnceCell::new();

/// Continuously tries to fetch the thread count lock
pub fn wait_for_tests<F: Fn() -> () + std::panic::RefUnwindSafe>(threads: usize, test_fn: F) {
    RUNNING_INFO.get_or_init(|| Mutex::new((0, 0)));
    let ri = RUNNING_INFO.get().unwrap();

    // wait until we have available resources to run the test
    loop {
        if let Ok(mut lock) = ri.try_lock() {
            if lock.0 + threads <= MAX_TEST_THREADS || lock.1 == 0 {
                tracing::info!("running_tests");

                lock.0 += threads;
                lock.1 += 1;
                break
            }
        }

        std::hint::spin_loop()
    }

    // run test capturing unwind
    let _ = std::panic::catch_unwind(|| test_fn());

    // decrement resources
    tracing::info!("test ran");
    loop {
        if let Ok(mut running_tests) = ri.try_lock() {
            tracing::info!("got running lock");
            running_tests.0 -= threads;
            running_tests.1 -= 1;
            tracing::info!("decremented resources");
            return
        } else {
            std::hint::spin_loop()
        }
    }
}
