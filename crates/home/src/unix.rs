use std::env;
use std::ffi::{CStr, OsString};
use std::mem;
use std::os::unix::prelude::OsStringExt;
use std::path::PathBuf;
use std::ptr;

pub fn home_dir_inner() -> Option<PathBuf> {
    return env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .or_else(|| unsafe { fallback() })
        .map(PathBuf::from);

    #[cfg(any(
        target_os = "android",
        target_os = "ios",
        target_os = "watchos",
        target_os = "emscripten",
        target_os = "redox",
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon"
    ))]
    unsafe fn fallback() -> Option<OsString> {
        None
    }
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "watchos",
        target_os = "emscripten",
        target_os = "redox",
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon"
    )))]
    unsafe fn fallback() -> Option<OsString> {
        let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512_usize,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(amt);
        let mut passwd: libc::passwd = mem::zeroed();
        let mut result = ptr::null_mut();
        match libc::getpwuid_r(
            libc::getuid(),
            &mut passwd,
            buf.as_mut_ptr(),
            buf.capacity(),
            &mut result,
        ) {
            0 if !result.is_null() => {
                let ptr = passwd.pw_dir as *const _;
                let bytes = CStr::from_ptr(ptr).to_bytes().to_vec();
                Some(OsStringExt::from_vec(bytes))
            }
            _ => None,
        }
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "watchos",
    target_os = "emscripten",
    target_os = "redox",
    target_os = "vxworks",
    target_os = "espidf",
    target_os = "horizon"
)))]
#[cfg(test)]
mod tests {
    use super::home_dir_inner;
    use std::env;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_with_without() {
        let oldhome: Option<PathBuf> = Some(env::var_os("HOME").unwrap().into());
        env::remove_var("HOME");
        assert_eq!(home_dir_inner(), oldhome);

        let home = Path::new("");
        env::set_var("HOME", home.as_os_str());
        assert_eq!(home_dir_inner(), oldhome);

        let home = Path::new("/home/foobarbaz");
        env::set_var("HOME", home.as_os_str());
        assert_eq!(home_dir_inner().as_deref(), Some(home));
    }
}
