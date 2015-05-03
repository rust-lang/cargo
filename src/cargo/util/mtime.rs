use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// A helper structure to represent the modification time of a file.
///
/// The actual value contined within is platform-specific and does not have the
/// same meaning across platforms, but comparisons and stringification can be
/// significant among platforms.
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Copy, Clone)]
pub struct MTime {
    seconds: u64,
    nanos: u32,
}

impl MTime {
    pub fn zero() -> MTime {
        MTime { seconds: 0, nanos: 0 }
    }

    pub fn of(p: &Path) -> io::Result<MTime> {
        let metadata = try!(fs::metadata(p));
        Ok(MTime::from(&metadata))
    }
}

impl<'a> From<&'a fs::Metadata> for MTime {
    #[cfg(unix)]
    fn from(meta: &'a fs::Metadata) -> MTime {
        use std::os::unix::prelude::*;
        let raw = meta.as_raw();
        // FIXME: currently there is a bug in the standard library where the
        //        nanosecond accessor is just accessing the seconds again, once
        //        that bug is fixed this should take nanoseconds into account.
        MTime { seconds: raw.mtime() as u64, nanos: 0 }
    }

    #[cfg(windows)]
    fn from(meta: &'a fs::Metadata) -> MTime {
        use std::os::windows::prelude::*;

        // Windows write times are in 100ns intervals, so do a little math to
        // get it into the right representation.
        let time = meta.last_write_time();
        MTime {
            seconds: time / (1_000_000_000 / 100),
            nanos: ((time % (1_000_000_000 / 100)) * 100) as u32,
        }
    }
}

impl fmt::Display for MTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:09}s", self.seconds, self.nanos)
    }
}
