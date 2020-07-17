#![feature(staged_api)]
#![stable(since = "1.0.0", feature = "dummy")]

#[stable(since = "1.0.0", feature = "dummy")]
pub use std::*;

#[stable(since = "1.0.0", feature = "dummy")]
pub fn custom_api() {}

#[cfg(feature = "feature1")]
#[stable(since = "1.0.0", feature = "dummy")]
pub fn conditional_function() {}
