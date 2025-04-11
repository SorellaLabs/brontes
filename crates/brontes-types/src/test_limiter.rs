//! Used to generate a queue for our tests to execute.
//! This is done so that we can limit the amount of cpu usage of the
//! tests to ensure we aren't nuking peoples computers / ci servers

use std::{str::FromStr, sync::Mutex};

use once_cell::sync::OnceCell;

/// Can be set using the env TEST_THREADS=<amount here>;
static MAX_TEST_THREADS: OnceCell<usize> = OnceCell::new();
static RUNNING_INFO: OnceCell<Mutex<(usize, usize)>> = OnceCell::new();

/// Continuously tries to fetch the thread count lock
pub fn wait_for_tests<F: Fn() + std::panic::RefUnwindSafe + std::panic::UnwindSafe>(
    threads: usize,
    test_fn: F,
) {
    let max_threads = MAX_TEST_THREADS.get_or_init(|| {
        std::env::var("TEST_THREADS")
            .map(|s| usize::from_str(&s).unwrap_or(12))
            .unwrap_or(12)
    });

    RUNNING_INFO.get_or_init(|| Mutex::new((0, 0)));
    let ri = RUNNING_INFO.get().unwrap();

    // wait until we have available resources to run the test
    loop {
        if let Ok(mut lock) = ri.try_lock() {
            if lock.0 + threads <= *max_threads || lock.1 == 0 {
                lock.0 += threads;
                lock.1 += 1;
                break;
            }
        }

        std::hint::spin_loop()
    }

    // run test capturing unwind
    let e = std::panic::catch_unwind(&test_fn);

    // decrement resources
    loop {
        if let Ok(mut running_tests) = ri.try_lock() {
            running_tests.0 -= threads;
            running_tests.1 -= 1;
            break;
        } else {
            std::hint::spin_loop()
        }
    }

    if e.is_err() {
        panic!("test failed");
    }
}
