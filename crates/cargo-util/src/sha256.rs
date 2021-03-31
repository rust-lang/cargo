use super::paths;
use anyhow::{Context, Result};
use crypto_hash::{Algorithm, Hasher};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

pub struct Sha256(Hasher);

impl Sha256 {
    pub fn new() -> Sha256 {
        let hasher = Hasher::new(Algorithm::SHA256);
        Sha256(hasher)
    }

    pub fn update(&mut self, bytes: &[u8]) -> &mut Sha256 {
        let _ = self.0.write_all(bytes);
        self
    }

    pub fn update_file(&mut self, mut file: &File) -> io::Result<&mut Sha256> {
        let mut buf = [0; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break Ok(self);
            }
            self.update(&buf[..n]);
        }
    }

    pub fn update_path<P: AsRef<Path>>(&mut self, path: P) -> Result<&mut Sha256> {
        let path = path.as_ref();
        let file = paths::open(path)?;
        self.update_file(&file)
            .with_context(|| format!("failed to read `{}`", path.display()))?;
        Ok(self)
    }

    pub fn finish(&mut self) -> [u8; 32] {
        let mut ret = [0u8; 32];
        let data = self.0.finish();
        ret.copy_from_slice(&data[..]);
        ret
    }

    pub fn finish_hex(&mut self) -> String {
        hex::encode(self.finish())
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}
