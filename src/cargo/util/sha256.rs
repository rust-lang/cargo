extern crate crypto_hash;
use self::crypto_hash::{Hasher,Algorithm};
use std::io::Write;

pub struct Sha256(Hasher);

impl Sha256 {
    pub fn new() -> Sha256 {
        let hasher = Hasher::new(Algorithm::SHA256);
        Sha256(hasher)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let _ = self.0.write_all(bytes);
    }

    pub fn finish(&mut self) -> [u8; 32] {
        let mut ret = [0u8; 32];
        let data = self.0.finish();
        ret.copy_from_slice(&data[..]);
        ret
    }
}
