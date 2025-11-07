//! Extension functions for [`std::sync::OnceLock`] / [`std::cell::OnceCell`]
//!
//! This adds polyfills for functionality in `lazycell` that is not stable within `std`.

pub trait OnceExt {
    type T;

    /// This might run `f` multiple times if different threads start initializing at once.
    fn try_borrow_with<F, E>(&self, f: F) -> Result<&Self::T, E>
    where
        F: FnOnce() -> Result<Self::T, E>;

    /// This might run `f` multiple times if different threads start initializing at once.
    fn try_borrow_mut_with<F, E>(&mut self, f: F) -> Result<&mut Self::T, E>
    where
        F: FnOnce() -> Result<Self::T, E>;

    fn replace(&mut self, new_value: Self::T) -> Option<Self::T>;

    fn filled(&self) -> bool;
}

impl<T> OnceExt for std::sync::OnceLock<T> {
    type T = T;

    fn try_borrow_with<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(value) = self.get() {
            return Ok(value);
        }

        // This is not how the unstable `OnceLock::get_or_try_init` works. That only starts `f` if
        // no other `f` is executing and the value is not initialized. However, correctly implementing that is
        // hard (one has properly handle panics in `f`) and not doable with the stable API of `OnceLock`.
        let value = f()?;
        // Another thread might have initialized `self` since we checked that `self.get()` returns `None`. If this is the case, `self.set()`
        // returns an error. We ignore it and return the value set by the other
        // thread.
        let _ = self.set(value);
        Ok(self.get().unwrap())
    }

    fn try_borrow_mut_with<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let value = if let Some(value) = self.take() {
            value
        } else {
            // This is not how the unstable `OnceLock::get_or_try_init` works. That only starts `f` if
            // no other `f` is executing and the value is not initialized. However, correctly implementing that is
            // hard (one has properly handle panics in `f`) and not doable with the stable API of `OnceLock`.
            f()?
        };
        // Another thread might have initialized `self` since we checked that `self.get()` returns `None`. If this is the case, `self.set()`
        // returns an error. We ignore it and return the value set by the other
        // thread.
        let _ = self.set(value);
        Ok(self.get_mut().unwrap())
    }

    fn replace(&mut self, new_value: T) -> Option<T> {
        if let Some(value) = self.get_mut() {
            Some(std::mem::replace(value, new_value))
        } else {
            let result = self.set(new_value);
            assert!(result.is_ok());
            None
        }
    }

    fn filled(&self) -> bool {
        self.get().is_some()
    }
}

impl<T> OnceExt for std::cell::OnceCell<T> {
    type T = T;

    fn try_borrow_with<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(value) = self.get() {
            return Ok(value);
        }

        // This is not how the unstable `OnceLock::get_or_try_init` works. That only starts `f` if
        // no other `f` is executing and the value is not initialized. However, correctly implementing that is
        // hard (one has properly handle panics in `f`) and not doable with the stable API of `OnceLock`.
        let value = f()?;
        // Another thread might have initialized `self` since we checked that `self.get()` returns `None`. If this is the case, `self.set()`
        // returns an error. We ignore it and return the value set by the other
        // thread.
        let _ = self.set(value);
        Ok(self.get().unwrap())
    }

    fn try_borrow_mut_with<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let value = if let Some(value) = self.take() {
            value
        } else {
            // This is not how the unstable `OnceLock::get_or_try_init` works. That only starts `f` if
            // no other `f` is executing and the value is not initialized. However, correctly implementing that is
            // hard (one has properly handle panics in `f`) and not doable with the stable API of `OnceLock`.
            f()?
        };
        // Another thread might have initialized `self` since we checked that `self.get()` returns `None`. If this is the case, `self.set()`
        // returns an error. We ignore it and return the value set by the other
        // thread.
        let _ = self.set(value);
        Ok(self.get_mut().unwrap())
    }

    fn replace(&mut self, new_value: T) -> Option<T> {
        if let Some(value) = self.get_mut() {
            Some(std::mem::replace(value, new_value))
        } else {
            let result = self.set(new_value);
            assert!(result.is_ok());
            None
        }
    }

    fn filled(&self) -> bool {
        self.get().is_some()
    }
}
