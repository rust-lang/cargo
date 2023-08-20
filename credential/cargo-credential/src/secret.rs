use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// A wrapper for values that should not be printed.
///
/// This type does not implement `Display`, and has a `Debug` impl that hides
/// the contained value.
///
/// ```
/// # use cargo_credential::Secret;
/// let token = Secret::from("super secret string");
/// assert_eq!(format!("{:?}", token), "Secret { inner: \"REDACTED\" }");
/// ```
///
/// Currently, we write a borrowed `Secret<T>` as `Secret<&T>`.
/// The [`as_deref`](Secret::as_deref) and [`to_owned`](Secret::to_owned) methods can
/// be used to convert back and forth between `Secret<String>` and `Secret<&str>`.
#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T> {
    inner: T,
}

impl<T> Secret<T> {
    /// Unwraps the contained value.
    ///
    /// Use of this method marks the boundary of where the contained value is
    /// hidden.
    pub fn expose(self) -> T {
        self.inner
    }

    /// Converts a `Secret<T>` to a `Secret<&T::Target>`.
    /// ```
    /// # use cargo_credential::Secret;
    /// let owned: Secret<String> = Secret::from(String::from("token"));
    /// let borrowed: Secret<&str> = owned.as_deref();
    /// ```
    pub fn as_deref(&self) -> Secret<&<T as Deref>::Target>
    where
        T: Deref,
    {
        Secret::from(self.inner.deref())
    }

    /// Converts a `Secret<T>` to a `Secret<&T>`.
    pub fn as_ref(&self) -> Secret<&T> {
        Secret::from(&self.inner)
    }

    /// Converts a `Secret<T>` to a `Secret<U>` by applying `f` to the contained value.
    pub fn map<U, F>(self, f: F) -> Secret<U>
    where
        F: FnOnce(T) -> U,
    {
        Secret::from(f(self.inner))
    }
}

impl<T: ToOwned + ?Sized> Secret<&T> {
    /// Converts a `Secret` containing a borrowed type to a `Secret` containing the
    /// corresponding owned type.
    /// ```
    /// # use cargo_credential::Secret;
    /// let borrowed: Secret<&str> = Secret::from("token");
    /// let owned: Secret<String> = borrowed.to_owned();
    /// ```
    pub fn to_owned(&self) -> Secret<<T as ToOwned>::Owned> {
        Secret::from(self.inner.to_owned())
    }
}

impl<T, E> Secret<Result<T, E>> {
    /// Converts a `Secret<Result<T, E>>` to a `Result<Secret<T>, E>`.
    pub fn transpose(self) -> Result<Secret<T>, E> {
        self.inner.map(|v| Secret::from(v))
    }
}

impl<T: AsRef<str>> Secret<T> {
    /// Checks if the contained value is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.as_ref().is_empty()
    }
}

impl<T> From<T> for Secret<T> {
    fn from(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret")
            .field("inner", &"REDACTED")
            .finish()
    }
}
