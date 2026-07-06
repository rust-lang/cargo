// Here we define wrappers for some of those types
#![allow(clippy::disallowed_types)]

pub use rustc_hash::FxHashMap as HashMap;
pub use rustc_hash::FxHashSet as HashSet;

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
pub type IndexSet<V> = indexmap::IndexSet<V, rustc_hash::FxBuildHasher>;
