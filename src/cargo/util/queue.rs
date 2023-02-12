use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// A simple, threadsafe, queue of items of type `T`
///
/// This is a sort of channel where any thread can push to a queue and any
/// thread can pop from a queue.
///
/// This supports both bounded and unbounded operations. [`push`] will never block,
/// and allows the queue to grow without bounds. [`push_bounded`] will block if
/// the queue is over capacity, and will resume once there is enough capacity.
///
/// [`push`]: Self::push
/// [`push_bounded`]: Self::push_bounded
pub struct Queue<T> {
    state: Mutex<State<T>>,
    popper_cv: Condvar,
    bounded_cv: Condvar,
    bound: usize,
}

struct State<T> {
    items: VecDeque<T>,
}

impl<T> Queue<T> {
    /// Creates a queue with a given bound.
    pub fn new(bound: usize) -> Queue<T> {
        Queue {
            state: Mutex::new(State {
                items: VecDeque::new(),
            }),
            popper_cv: Condvar::new(),
            bounded_cv: Condvar::new(),
            bound,
        }
    }

    /// Pushes an item onto the queue, regardless of the capacity of the queue.
    pub fn push(&self, item: T) {
        self.state.lock().unwrap().items.push_back(item);
        self.popper_cv.notify_one();
    }

    /// Pushes an item onto the queue, blocking if the queue is full.
    pub fn push_bounded(&self, item: T) {
        let locked_state = self.state.lock().unwrap();
        let mut state = self
            .bounded_cv
            .wait_while(locked_state, |s| s.items.len() >= self.bound)
            .unwrap();
        state.items.push_back(item);
        self.popper_cv.notify_one();
    }

    /// Pops an item from the queue, blocking if the queue is empty.
    pub fn pop(&self, timeout: Duration) -> Option<T> {
        let (mut state, result) = self
            .popper_cv
            .wait_timeout_while(self.state.lock().unwrap(), timeout, |s| s.items.is_empty())
            .unwrap();
        if result.timed_out() {
            None
        } else {
            let value = state.items.pop_front()?;
            if state.items.len() < self.bound {
                // Assumes threads cannot be canceled.
                self.bounded_cv.notify_one();
            }
            Some(value)
        }
    }

    /// Pops all items from the queue without blocking.
    pub fn try_pop_all(&self) -> Vec<T> {
        let mut state = self.state.lock().unwrap();
        let result = state.items.drain(..).collect();
        self.bounded_cv.notify_all();
        result
    }
}
