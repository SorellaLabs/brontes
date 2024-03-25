//! Used to generate a queue for our tests to execute.
//! This is done so that we can limit the amount of cpu usage of the
//! tests to ensure we aren't nuking peoples computers / ci servers

use std::sync::Mutex;

use once_cell::sync::OnceCell;

const MAX_TEST_THREADS: usize = 12;
static RUNNING_THREAD_COUNT: OnceCell<Mutex<usize>> = OnceCell::new();
static RUNNING_TEST_COUNT: OnceCell<Mutex<usize>> = OnceCell::new();

/// Continuously tries to fetch the thread count lock
pub fn wait_for_tests<F: Fn() -> () + std::panic::RefUnwindSafe>(threads: usize, test_fn: F) {
    RUNNING_THREAD_COUNT.get_or_init(|| Mutex::new(0));
    RUNNING_TEST_COUNT.get_or_init(|| Mutex::new(0));

    let thc = RUNNING_THREAD_COUNT.get().unwrap();
    let tc = RUNNING_TEST_COUNT.get().unwrap();

    // wait until we have available resources to run the test
    loop {
        if let Ok(mut lock) = thc.try_lock() {
            let mut test_count = tc.lock().unwrap();
            if *lock + threads < MAX_TEST_THREADS || *test_count == 0 {
                *test_count += 1;
                *lock += threads;

                break
            }
        }

        std::hint::spin_loop()
    }

    // run test capturing unwind
    let _ = std::panic::catch_unwind(|| test_fn());

    // decrement resources
    let mut running_tests = tc.lock().unwrap();
    *running_tests -= 1;
    let mut thread_count = thc.lock().unwrap();
    *thread_count -= threads;
}
