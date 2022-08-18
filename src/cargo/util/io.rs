use std::io::{self, Read, Take};

#[derive(Debug)]
pub struct LimitErrorReader<R> {
    inner: Take<R>,
}

impl<R: Read> LimitErrorReader<R> {
    pub fn new(r: R, limit: u64) -> LimitErrorReader<R> {
        LimitErrorReader {
            inner: r.take(limit),
        }
    }
}

impl<R: Read> Read for LimitErrorReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.read(buf) {
            Ok(0) if self.inner.limit() == 0 => Err(io::Error::new(
                io::ErrorKind::Other,
                "maximum limit reached when reading",
            )),
            e => e,
        }
    }
}

