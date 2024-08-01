//! A hasher that produces the same values across releases and platforms.
//!
//! This is a wrapper around [`rustc_stable_hash::StableHasher`].

pub use rustc_stable_hash::StableSipHasher128 as StableHasher;
