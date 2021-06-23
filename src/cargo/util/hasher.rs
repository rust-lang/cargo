//! Implementation of a hasher that produces the same values across releases.
//!
//! The hasher should be fast and have a low chance of collisions (but is not
//! sufficient for cryptographic purposes).
#![allow(deprecated)]

use std::hash::{Hasher, SipHasher};

#[derive(Default)]
pub struct StableHasher(SipHasher);

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0.finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }
}
