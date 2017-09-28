extern crate sha2;
use self::sha2::Digest;

pub struct Sha256(sha2::Sha256);

impl Sha256 {
    pub fn new() -> Sha256 {
        let hasher = sha2::Sha256::new();
        Sha256(hasher)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let _ = self.0.input(bytes);
    }

    pub fn finish(&mut self) -> [u8; 32] {
        let mut ret = [0u8; 32];
        let data = self.0.result();
        ret.copy_from_slice(&data[..]);

        // sha2::Sha256::result() doesn't reset its buffer
        // Reset manually by replacing hasher
        self.0 = sha2::Sha256::new();

        ret
    }
}
