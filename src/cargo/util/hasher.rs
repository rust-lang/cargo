//! A hasher that produces the same values across releases and platforms.
//!
//! This is a wrapper around [`rustc_stable_hash::StableHasher`].

pub struct StableHasher(rustc_stable_hash::StableHasher);

impl StableHasher {
    pub fn new() -> StableHasher {
        StableHasher(rustc_stable_hash::StableHasher::new())
    }

    pub fn finish(self) -> u64 {
        self.0.finalize().0
    }
}

impl std::hash::Hasher for StableHasher {
    fn finish(&self) -> u64 {
        panic!("call StableHasher::finish instead");
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }
}
