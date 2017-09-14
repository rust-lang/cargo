//! A lazily fill Cell, but with frozen contents.
//!
//! With a `RefCell`, the inner contents cannot be borrowed for the lifetime of
//! the entire object, but only of the borrows returned. A `LazyCell` is a
//! variation on `RefCell` which allows borrows tied to the lifetime of the
//! outer object.
//!
//! The limitation of a `LazyCell` is that after initialized, it can never be
//! modified unless you've otherwise got a `&mut` reference

use std::cell::UnsafeCell;

#[derive(Debug)]
pub struct LazyCell<T> {
    inner: UnsafeCell<Option<T>>,
}

impl<T> LazyCell<T> {
    /// Creates a new empty lazy cell.
    pub fn new() -> LazyCell<T> {
        LazyCell { inner: UnsafeCell::new(None) }
    }

    /// Put a value into this cell.
    ///
    /// This function will fail if the cell has already been filled.
    pub fn fill(&self, t: T) -> Result<(), T> {
        unsafe {
            let slot = self.inner.get();
            if (*slot).is_none() {
                *slot = Some(t);
                Ok(())
            } else {
                Err(t)
            }
        }
    }

    /// Borrows the contents of this lazy cell for the duration of the cell
    /// itself.
    ///
    /// This function will return `Some` if the cell has been previously
    /// initialized, and `None` if it has not yet been initialized.
    pub fn borrow(&self) -> Option<&T> {
        unsafe {
            (*self.inner.get()).as_ref()
        }
    }

    /// Same as `borrow`, but the mutable version
    pub fn borrow_mut(&mut self) -> Option<&mut T> {
        unsafe {
            (*self.inner.get()).as_mut()
        }
    }

    /// Consumes this `LazyCell`, returning the underlying value.
    pub fn into_inner(self) -> Option<T> {
        unsafe {
            self.inner.into_inner()
        }
    }

    /// Borrows the contents of this lazy cell, initializing it if necessary.
    pub fn get_or_try_init<Error, F>(&self, init: F) -> Result<&T, Error>
        where F: FnOnce() -> Result<T, Error>
    {
        if self.borrow().is_none() {
            if self.fill(init()?).is_err() {
                unreachable!();
            }
        }
        Ok(self.borrow().unwrap())
    }
}
