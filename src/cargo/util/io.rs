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

#[cfg(test)]
mod tests {
    use super::LimitErrorReader;

    use std::io::Read;

    #[test]
    fn under_the_limit() {
        let buf = &[1; 7][..];
        let mut r = LimitErrorReader::new(buf, 8);
        let mut out = Vec::new();
        assert!(matches!(r.read_to_end(&mut out), Ok(7)));
        assert_eq!(buf, out.as_slice());
    }

    #[test]
    #[should_panic = "maximum limit reached when reading"]
    fn over_the_limit() {
        let buf = &[1; 8][..];
        let mut r = LimitErrorReader::new(buf, 8);
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
    }
}
