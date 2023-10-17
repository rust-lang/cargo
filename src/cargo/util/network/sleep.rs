//! Utility for tracking network requests that will be retried in the future.

use core::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

/// A tracker for network requests that have failed, and are awaiting to be
/// retried in the future.
pub struct SleepTracker<T> {
    /// This is a priority queue that tracks the time when the next sleeper
    /// should awaken (based on the [`Sleeper::wakeup`] property).
    heap: BinaryHeap<Sleeper<T>>,
}

/// An individual network request that is waiting to be retried in the future.
struct Sleeper<T> {
    /// The time when this requests should be retried.
    wakeup: Instant,
    /// Information about the network request.
    data: T,
}

impl<T> PartialEq for Sleeper<T> {
    fn eq(&self, other: &Sleeper<T>) -> bool {
        self.wakeup == other.wakeup
    }
}

impl<T> PartialOrd for Sleeper<T> {
    fn partial_cmp(&self, other: &Sleeper<T>) -> Option<Ordering> {
        // This reverses the comparison so that the BinaryHeap tracks the
        // entry with the *lowest* wakeup time.
        Some(other.wakeup.cmp(&self.wakeup))
    }
}

impl<T> Eq for Sleeper<T> {}

impl<T> Ord for Sleeper<T> {
    fn cmp(&self, other: &Sleeper<T>) -> Ordering {
        self.wakeup.cmp(&other.wakeup)
    }
}

impl<T> SleepTracker<T> {
    pub fn new() -> SleepTracker<T> {
        SleepTracker {
            heap: BinaryHeap::new(),
        }
    }

    /// Adds a new download that should be retried in the future.
    pub fn push(&mut self, sleep: u64, data: T) {
        self.heap.push(Sleeper {
            wakeup: Instant::now()
                .checked_add(Duration::from_millis(sleep))
                .expect("instant should not wrap"),
            data,
        });
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Returns any downloads that are ready to go now.
    pub fn to_retry(&mut self) -> Vec<T> {
        let now = Instant::now();
        let mut result = Vec::new();
        while let Some(next) = self.heap.peek() {
            if next.wakeup < now {
                result.push(self.heap.pop().unwrap().data);
            } else {
                break;
            }
        }
        result
    }

    /// Returns the time when the next download is ready to go.
    ///
    /// Returns None if there are no sleepers remaining.
    pub fn time_to_next(&self) -> Option<Duration> {
        self.heap
            .peek()
            .map(|s| s.wakeup.saturating_duration_since(Instant::now()))
    }
}

#[test]
fn returns_in_order() {
    let mut s = SleepTracker::new();
    s.push(3, 3);
    s.push(1, 1);
    s.push(6, 6);
    s.push(5, 5);
    s.push(2, 2);
    s.push(10000, 10000);
    assert_eq!(s.len(), 6);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(s.to_retry(), &[1, 2, 3, 5, 6]);
}
