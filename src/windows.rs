#![cfg(windows)]

use std::env;
use std::ffi::OsString;
use std::mem::MaybeUninit;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;
use std::slice;

use winapi::shared::minwindef::MAX_PATH;
use winapi::shared::winerror::S_OK;
use winapi::um::shlobj::{SHGetFolderPathW, CSIDL_PROFILE};

pub fn home_dir_inner() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(home_dir_crt)
}

#[cfg(not(target_vendor = "uwp"))]
fn home_dir_crt() -> Option<PathBuf> {
    unsafe {
        let mut path: [MaybeUninit<u16>; MAX_PATH] = MaybeUninit::uninit().assume_init();
        let ptr = path.as_mut_ptr() as *mut u16;
        match SHGetFolderPathW(ptr::null_mut(), CSIDL_PROFILE, ptr::null_mut(), 0, ptr) {
            S_OK => {
                let ptr = path.as_ptr() as *const u16;
                let len = wcslen(ptr);
                let path = slice::from_raw_parts(ptr, len);
                let s = OsString::from_wide(path);
                Some(PathBuf::from(s))
            }
            _ => None,
        }
    }
}

#[cfg(target_vendor = "uwp")]
fn home_dir_crt() -> Option<PathBuf> {
    None
}

extern "C" {
    fn wcslen(buf: *const u16) -> usize;
}

#[cfg(not(target_vendor = "uwp"))]
#[cfg(test)]
mod tests {
    use super::home_dir_inner;
    use std::env;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_with_without() {
        let olduserprofile = env::var_os("USERPROFILE").unwrap();

        env::remove_var("HOME");
        env::remove_var("USERPROFILE");

        assert_eq!(home_dir_inner(), Some(PathBuf::from(olduserprofile)));

        let home = Path::new(r"C:\Users\foo tar baz");

        env::set_var("HOME", home.as_os_str());
        assert_ne!(home_dir_inner().as_deref(), Some(home));

        env::set_var("USERPROFILE", home.as_os_str());
        assert_eq!(home_dir_inner().as_deref(), Some(home));
    }
}
