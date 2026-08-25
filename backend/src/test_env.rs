//! Serialization for tests that set environment variables.
//!
//! `XDG_CACHE_HOME` and `XDG_DATA_HOME` are process-wide, and the test harness
//! runs tests in threads. Any test that points one of them at a temporary
//! directory has to hold this lock for as long as it depends on the value, or
//! it will read another test's directory - which is how the crypto tests first
//! failed, with errors that looked like real permission bugs.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Takes the environment lock, recovering it if a previous test panicked while
/// holding it: the next test's failure should be its own, not a cascade.
pub fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
