//! A hasher that produces the same values across releases and platforms.
//!
//! The hasher should be fast and have a low chance of collisions (but is not
//! sufficient for cryptographic purposes).

pub use rustc_stable_hash::StableSipHasher128 as StableHasher;
