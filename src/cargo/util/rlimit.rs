#[cfg(unix)]
use libc::{RLIMIT_NOFILE, getrlimit, rlimit, setrlimit};

use crate::CargoResult;

pub struct ResourceLimits {
    pub soft_limit: u64,
    pub hard_limit: u64,
}

#[cfg(unix)]
pub fn get_max_file_descriptors() -> CargoResult<ResourceLimits> {
    let mut rlim = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    let result = unsafe { getrlimit(RLIMIT_NOFILE, &mut rlim) };
    if result != 0 {
        return Err(anyhow::Error::from(std::io::Error::last_os_error())
            .context("Failed to get rlimit with error code"));
    }

    return Ok(ResourceLimits {
        soft_limit: rlim.rlim_cur,
        hard_limit: rlim.rlim_max,
    });
}

#[cfg(windows)]
pub fn get_max_file_descriptors() -> CargoResult<ResourceLimits> {
    let soft_limit = windows::getmaxstdio() as u64;

    return Ok(ResourceLimits {
        soft_limit,
        // Windows does not provide a way to get the hard limit so we return a estimated max.
        // This is likely less than max for some systems but should be supported by most and is
        // likely high enough that most projects do not run out of file descriptors.
        // See: https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/setmaxstdio?view=msvc-170#remarks
        hard_limit: 8192,
    });
}

#[cfg(unix)]
pub fn set_max_file_descriptors(limits: ResourceLimits) -> CargoResult<()> {
    let rlim = rlimit {
        rlim_cur: limits.soft_limit,
        rlim_max: limits.hard_limit,
    };
    let result = unsafe { setrlimit(RLIMIT_NOFILE, &rlim) };
    if result != 0 {
        return Err(anyhow::Error::from(std::io::Error::last_os_error())
            .context("Failed to set rlimit with error code"));
    }

    return Ok(());
}

#[cfg(windows)]
pub fn set_max_file_descriptors(limits: ResourceLimits) -> CargoResult<()> {
    windows::setmaxstdio(limits.soft_limit as u32)?;
    return Ok(());
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn _setmaxstdio(new_max: c_int) -> c_int;
        fn _getmaxstdio() -> c_int;
    }

    /// Sets a maximum for the number of simultaneously open files at the stream I/O level.
    ///
    /// See <https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/setmaxstdio?view=msvc-170>
    pub fn setmaxstdio(new_max: u32) -> io::Result<u32> {
        // A negative `new_max` will cause EINVAL.
        // A negative `ret` should never appear.
        // It is safe even if the return value is wrong.
        #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
        unsafe {
            let ret = _setmaxstdio(new_max as c_int);
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(ret as u32)
        }
    }

    /// Returns the number of simultaneously open files permitted at the stream I/O level.
    ///
    /// See <https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/getmaxstdio?view=msvc-170>
    #[must_use]
    pub fn getmaxstdio() -> u32 {
        // A negative `ret` should never appear.
        // It is safe even if the return value is wrong.
        #[allow(clippy::cast_sign_loss)]
        unsafe {
            let ret = _getmaxstdio();
            debug_assert!(ret >= 0);
            ret as u32
        }
    }
}
