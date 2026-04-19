use futures::{FutureExt, future::LocalBoxFuture, stream::FuturesUnordered};
use std::{collections::HashMap, hash::Hash, ops::Deref, task::Poll};

/// A local (!Send) adapter for caching and executing an async method
/// from a non-async context.
///
/// The `self_parameter`, `key`, and successful (Ok) results must all be cheap to `clone`.
///
/// Ensures at most one in-flight computation per key. Results are:
/// - cached on success
/// - not retained on error
pub struct LocalPollAdapter<'a, S, K, R> {
    pool: FuturesUnordered<LocalBoxFuture<'a, (K, R)>>,
    cache: HashMap<K, Poll<R>>,
    self_parameter: S,
}

impl<'a, S, K, V, E> LocalPollAdapter<'a, S, K, Result<V, E>>
where
    S: Clone + Deref + 'a,
    K: Clone + Hash + Eq + 'a,
    V: Clone,
{
    pub fn new(self_parameter: S) -> Self {
        Self {
            pool: FuturesUnordered::new(),
            cache: HashMap::new(),
            self_parameter,
        }
    }

    /// Polls the result for `key`, spawning work if needed.
    ///
    /// If this function returns [`Poll::Pending`], call [`LocalPollAdapter::wait`]
    /// to execute the work, then call this function again with the same key
    /// to pick up the result.
    ///
    /// Futures that complete immediately are not queued.
    pub fn poll<F>(&mut self, f: F, key: K) -> Poll<Result<V, E>>
    where
        F: AsyncFn(&S::Target, &K) -> Result<V, E> + 'a,
    {
        match self.cache.get(&key) {
            // We have a cached success value, clone it and return.
            Some(Poll::Ready(Ok(v))) => return Poll::Ready(Ok(v.clone())),
            // We have a cached error value, remove it and return.
            // Errors are not Clone, so they are only stored once.
            Some(Poll::Ready(Err(_))) => return self.cache.remove(&key).unwrap(),
            // This key is already pending.
            Some(Poll::Pending) => return Poll::Pending,
            // Looks like we have work to do!
            None => {}
        }

        // Created a pinned future that executes the function,
        // returning the key and the result.
        let mut future = {
            let key = key.clone();
            let self_parameter = self.self_parameter.clone();
            async move {
                let v = f(self_parameter.deref(), &key).await;
                (key, v)
            }
            .boxed_local()
        };

        // Attempt to run the future immediately. If it has no `await` yields,
        // it will return here.
        if let Some((k, v)) = (&mut future).now_or_never() {
            if let Ok(success) = &v {
                // Only cache successful results.
                self.cache.insert(k, Poll::Ready(Ok(success.clone())));
            }
            return Poll::Ready(v);
        }

        // Insert Pending into the cache so we avoid queuing the same future twice.
        self.cache.insert(key.clone(), Poll::Pending);

        // Add the future to the pending queue.
        self.pool.push(future);
        Poll::Pending
    }

    /// Returns the number of pending futures.
    pub fn pending_count(&self) -> usize {
        self.pool.len()
    }

    /// Run all pending futures. Returns true if there was no work to do.
    pub fn wait(&mut self) -> bool {
        let is_empty = self.pool.is_empty();
        for (k, v) in crate::util::block_on_stream(&mut self.pool) {
            *self
                .cache
                .get_mut(&k)
                .expect("all pending work is in the cache") = Poll::Ready(v);
        }
        is_empty
    }
}

#[cfg(test)]
mod tests {
    use super::LocalPollAdapter;
    use std::{rc::Rc, task::Poll};

    /// Future that yields once.
    fn yield_once() -> impl std::future::Future<Output = ()> {
        let mut yielded = false;

        std::future::poll_fn(move |cx| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
    }

    struct Thing {}

    impl Thing {
        async fn widen(&self, i: &i32) -> Result<i64, ()> {
            if *i > 10 {
                // Big numbers take longer to process (need to test futures that yield).
                yield_once().await;
            }
            if *i % 2 != 0 {
                // Odd numbers are not supported (need to test errors).
                return Err(());
            }
            Ok(*i as i64)
        }
    }

    /// Poll wrapper around `Thing`
    struct PolledThing<'a> {
        poller: LocalPollAdapter<'a, Rc<Thing>, i32, Result<i64, ()>>,
    }

    impl<'a> PolledThing<'a> {
        fn new() -> Self {
            Self {
                poller: LocalPollAdapter::new(Rc::new(Thing {})),
            }
        }

        // Non-async version of the widen method.
        fn widen(&mut self, i: &i32) -> Poll<Result<i64, ()>> {
            self.poller.poll(Thing::widen, i.clone())
        }

        fn wait(&mut self) -> bool {
            self.poller.wait()
        }
    }

    #[test]
    fn immediate_success() {
        let mut p = PolledThing::new();
        assert_eq!(p.widen(&2), Poll::Ready(Ok(2)));
        assert!(p.wait());
    }

    #[test]
    fn immediate_error() {
        let mut p = PolledThing::new();
        assert_eq!(p.widen(&1), Poll::Ready(Err(())));
        assert!(p.wait());
    }

    #[test]
    fn deferred_error() {
        let mut p = PolledThing::new();
        assert_eq!(p.widen(&1001), Poll::Pending);
        assert!(!p.wait());
        assert_eq!(p.widen(&1001), Poll::Ready(Err(())));
        assert!(p.wait());
        // Errors are not cached
        assert_eq!(p.widen(&1001), Poll::Pending);
        assert!(!p.wait());
        assert_eq!(p.widen(&1001), Poll::Ready(Err(())));
        assert!(p.wait());
    }

    #[test]
    fn deferred_success() {
        let mut p = PolledThing::new();
        assert_eq!(p.widen(&50), Poll::Pending);
        assert!(!p.wait());
        assert_eq!(p.widen(&50), Poll::Ready(Ok(50)));
        assert!(p.wait());
        // Success is cached.
        assert_eq!(p.widen(&50), Poll::Ready(Ok(50)));
        assert!(p.wait());
    }
}
