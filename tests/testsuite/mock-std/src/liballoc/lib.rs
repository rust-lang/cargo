#![feature(staged_api)]
#![stable(since = "1.0.0", feature = "dummy")]

extern crate alloc;

#[stable(since = "1.0.0", feature = "dummy")]
pub use alloc::*;

#[stable(since = "1.0.0", feature = "dummy")]
pub fn custom_api() {
}
