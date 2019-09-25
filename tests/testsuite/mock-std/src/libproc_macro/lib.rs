#![feature(staged_api)]
#![stable(since = "1.0.0", feature = "dummy")]

extern crate proc_macro;

#[stable(since = "1.0.0", feature = "dummy")]
pub use proc_macro::*;

#[stable(since = "1.0.0", feature = "dummy")]
pub fn custom_api() {
}
