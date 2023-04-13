#![feature(staged_api)]
#![stable(since = "1.0.0", feature = "dummy")]


extern crate proc_macro;

// Don't re-export everything in the root so that the mock std can be distinguished from the real one.
#[stable(since = "1.0.0", feature = "dummy")]
pub mod exported {
    #[stable(since = "1.0.0", feature = "dummy")]
    pub use proc_macro::*;
}

#[stable(since = "1.0.0", feature = "dummy")]
pub fn custom_api() {
}
