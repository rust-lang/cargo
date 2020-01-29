use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// A simple, threadsafe, queue of items of type `T`
///
/// This is a sort of channel where any thread can push to a queue and any
/// thread can pop from a queue. Currently queues have infinite capacity where
/// `push` will never block but `pop` will block.
pub struct Queue<T> {
    state: Mutex<State<T>>,
    condvar: Condvar,
}

struct State<T> {
    items: VecDeque<T>,
}

impl<T> Queue<T> {
    pub fn new() -> Queue<T> {
        Queue {
            state: Mutex::new(State {
                items: VecDeque::new(),
            }),
            condvar: Condvar::new(),
        }
    }

    pub fn push(&self, item: T) {
        self.state.lock().unwrap().items.push_back(item);
        self.condvar.notify_one();
    }

    pub fn pop(&self, timeout: Duration) -> Option<T> {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        while state.items.is_empty() {
            let elapsed = now.elapsed();
            if elapsed >= timeout {
                break;
            }
            let (lock, result) = self.condvar.wait_timeout(state, timeout - elapsed).unwrap();
            state = lock;
            if result.timed_out() {
                break;
            }
        }
        state.items.pop_front()
    }

    pub fn try_pop(&self) -> Option<T> {
        self.state.lock().unwrap().items.pop_front()
    }
}
