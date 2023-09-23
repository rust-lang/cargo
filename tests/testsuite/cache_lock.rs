//! Tests for `CacheLock`.

use crate::config::ConfigBuilder;
use cargo::util::cache_lock::{CacheLockMode, CacheLocker};
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::{retry, thread_wait_timeout, threaded_timeout};
use std::thread::JoinHandle;

/// Helper to verify that it is OK to acquire the given lock (it shouldn't block).
fn verify_lock_is_ok(mode: CacheLockMode) {
    let root = paths::root();
    threaded_timeout(10, move || {
        let config = ConfigBuilder::new().root(root).build();
        let locker = CacheLocker::new();
        // This would block if it is held.
        let _lock = locker.lock(&config, mode).unwrap();
        assert!(locker.is_locked(mode));
    });
}

/// Helper to acquire two locks from the same locker.
fn a_b_nested(a: CacheLockMode, b: CacheLockMode) {
    let config = ConfigBuilder::new().build();
    let locker = CacheLocker::new();
    let lock1 = locker.lock(&config, a).unwrap();
    assert!(locker.is_locked(a));
    let lock2 = locker.lock(&config, b).unwrap();
    assert!(locker.is_locked(b));
    drop(lock2);
    drop(lock1);
    // Verify locks were unlocked.
    verify_lock_is_ok(CacheLockMode::Shared);
    verify_lock_is_ok(CacheLockMode::DownloadExclusive);
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
}

/// Helper to acquire two locks from separate lockers, verifying that they
/// don't block each other.
fn a_then_b_separate_not_blocked(a: CacheLockMode, b: CacheLockMode, verify: CacheLockMode) {
    let config = ConfigBuilder::new().build();
    let locker1 = CacheLocker::new();
    let lock1 = locker1.lock(&config, a).unwrap();
    assert!(locker1.is_locked(a));
    let locker2 = CacheLocker::new();
    let lock2 = locker2.lock(&config, b).unwrap();
    assert!(locker2.is_locked(b));
    let thread = verify_lock_would_block(verify);
    // Unblock the thread.
    drop(lock1);
    drop(lock2);
    // Verify the thread is unblocked.
    thread_wait_timeout::<()>(100, thread);
}

/// Helper to acquire two locks from separate lockers, verifying that the
/// second one blocks.
fn a_then_b_separate_blocked(a: CacheLockMode, b: CacheLockMode) {
    let config = ConfigBuilder::new().build();
    let locker = CacheLocker::new();
    let lock = locker.lock(&config, a).unwrap();
    assert!(locker.is_locked(a));
    let thread = verify_lock_would_block(b);
    // Unblock the thread.
    drop(lock);
    // Verify the thread is unblocked.
    thread_wait_timeout::<()>(100, thread);
}

/// Helper to verify that acquiring the given mode would block.
///
/// Always call `thread_wait_timeout` on the result.
#[must_use]
fn verify_lock_would_block(mode: CacheLockMode) -> JoinHandle<()> {
    let root = paths::root();
    // Spawn a thread that will block on the lock.
    let thread = std::thread::spawn(move || {
        let config = ConfigBuilder::new().root(root).build();
        let locker2 = CacheLocker::new();
        let lock2 = locker2.lock(&config, mode).unwrap();
        assert!(locker2.is_locked(mode));
        drop(lock2);
    });
    // Verify that it blocked.
    retry(100, || {
        if let Ok(s) = std::fs::read_to_string(paths::root().join("shell.out")) {
            if s.trim().starts_with("Blocking waiting for file lock on") {
                return Some(());
            } else {
                eprintln!("unexpected output: {s}");
                // Try again, it might have been partially written.
            }
        }
        None
    });
    thread
}

#[test]
fn new_is_unlocked() {
    let locker = CacheLocker::new();
    assert!(!locker.is_locked(CacheLockMode::Shared));
    assert!(!locker.is_locked(CacheLockMode::DownloadExclusive));
    assert!(!locker.is_locked(CacheLockMode::MutateExclusive));
}

#[cargo_test]
fn multiple_shared() {
    // Test that two nested shared locks from the same locker are safe to acquire.
    a_b_nested(CacheLockMode::Shared, CacheLockMode::Shared);
}

#[cargo_test]
fn multiple_shared_separate() {
    // Test that two independent shared locks are safe to acquire at the same time.
    a_then_b_separate_not_blocked(
        CacheLockMode::Shared,
        CacheLockMode::Shared,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
fn multiple_download() {
    // That that two nested download locks from the same locker are safe to acquire.
    a_b_nested(
        CacheLockMode::DownloadExclusive,
        CacheLockMode::DownloadExclusive,
    );
}

#[cargo_test]
fn multiple_mutate() {
    // That that two nested mutate locks from the same locker are safe to acquire.
    a_b_nested(
        CacheLockMode::MutateExclusive,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
#[should_panic(expected = "lock is not allowed")]
fn download_then_shared() {
    // This sequence is not supported.
    a_b_nested(CacheLockMode::DownloadExclusive, CacheLockMode::Shared);
}

#[cargo_test]
#[should_panic(expected = "lock upgrade from shared to exclusive not supported")]
fn shared_then_mutate() {
    // This sequence is not supported.
    a_b_nested(CacheLockMode::Shared, CacheLockMode::MutateExclusive);
}

#[cargo_test]
fn shared_then_download() {
    a_b_nested(CacheLockMode::Shared, CacheLockMode::DownloadExclusive);
    // Verify drop actually unlocked.
    verify_lock_is_ok(CacheLockMode::DownloadExclusive);
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
}

#[cargo_test]
fn mutate_then_shared() {
    a_b_nested(CacheLockMode::MutateExclusive, CacheLockMode::Shared);
    // Verify drop actually unlocked.
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
}

#[cargo_test]
fn download_then_mutate() {
    a_b_nested(
        CacheLockMode::DownloadExclusive,
        CacheLockMode::MutateExclusive,
    );
    // Verify drop actually unlocked.
    verify_lock_is_ok(CacheLockMode::DownloadExclusive);
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
}

#[cargo_test]
fn mutate_then_download() {
    a_b_nested(
        CacheLockMode::MutateExclusive,
        CacheLockMode::DownloadExclusive,
    );
    // Verify drop actually unlocked.
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
    verify_lock_is_ok(CacheLockMode::DownloadExclusive);
}

#[cargo_test]
fn readonly() {
    // In a permission denied situation, it should still allow a lock. It just
    // silently behaves as-if it was locked.
    let cargo_home = paths::home().join(".cargo");
    std::fs::create_dir_all(&cargo_home).unwrap();
    let mut perms = std::fs::metadata(&cargo_home).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&cargo_home, perms).unwrap();
    let config = ConfigBuilder::new().build();
    let locker = CacheLocker::new();
    for mode in [
        CacheLockMode::Shared,
        CacheLockMode::DownloadExclusive,
        CacheLockMode::MutateExclusive,
    ] {
        let _lock1 = locker.lock(&config, mode).unwrap();
        // Make sure it can recursively acquire the lock, too.
        let _lock2 = locker.lock(&config, mode).unwrap();
    }
}

#[cargo_test]
fn download_then_shared_separate() {
    a_then_b_separate_not_blocked(
        CacheLockMode::DownloadExclusive,
        CacheLockMode::Shared,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
fn shared_then_download_separate() {
    a_then_b_separate_not_blocked(
        CacheLockMode::Shared,
        CacheLockMode::DownloadExclusive,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
fn multiple_download_separate() {
    // Test that with two independent download locks, the second blocks until
    // the first is released.
    a_then_b_separate_blocked(
        CacheLockMode::DownloadExclusive,
        CacheLockMode::DownloadExclusive,
    );
}

#[cargo_test]
fn multiple_mutate_separate() {
    // Test that with two independent mutate locks, the second blocks until
    // the first is released.
    a_then_b_separate_blocked(
        CacheLockMode::MutateExclusive,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
fn shared_then_mutate_separate() {
    a_then_b_separate_blocked(CacheLockMode::Shared, CacheLockMode::MutateExclusive);
}

#[cargo_test]
fn download_then_mutate_separate() {
    a_then_b_separate_blocked(
        CacheLockMode::DownloadExclusive,
        CacheLockMode::MutateExclusive,
    );
}

#[cargo_test]
fn mutate_then_download_separate() {
    a_then_b_separate_blocked(
        CacheLockMode::MutateExclusive,
        CacheLockMode::DownloadExclusive,
    );
}

#[cargo_test]
fn mutate_then_shared_separate() {
    a_then_b_separate_blocked(CacheLockMode::MutateExclusive, CacheLockMode::Shared);
}

#[cargo_test(ignore_windows = "no method to prevent creating or locking a file")]
fn mutate_err_is_atomic() {
    // Verifies that when getting a mutate lock, that if the first lock
    // succeeds, but the second one fails, that the first lock is released.
    let config = ConfigBuilder::new().build();
    let locker = CacheLocker::new();
    let cargo_home = config.home().as_path_unlocked();
    let cache_path = cargo_home.join(".package-cache");
    // This is a hacky way to force an error acquiring the download lock. By
    // making it a directory, it is unable to open it.
    // TODO: Unfortunately this doesn't work on Windows. I don't have any
    // ideas on how to simulate an error on Windows.
    cache_path.mkdir_p();
    match locker.lock(&config, CacheLockMode::MutateExclusive) {
        Ok(_) => panic!("did not expect lock to succeed"),
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(msg.contains("failed to open:"), "{msg}");
        }
    }
    assert!(!locker.is_locked(CacheLockMode::MutateExclusive));
    assert!(!locker.is_locked(CacheLockMode::DownloadExclusive));
    assert!(!locker.is_locked(CacheLockMode::Shared));
    cache_path.rm_rf();
    verify_lock_is_ok(CacheLockMode::DownloadExclusive);
    verify_lock_is_ok(CacheLockMode::Shared);
    verify_lock_is_ok(CacheLockMode::MutateExclusive);
}
