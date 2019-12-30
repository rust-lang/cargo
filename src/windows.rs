#![cfg(windows)]

use std::env;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use winapi::shared::minwindef::{DWORD, MAX_PATH};
use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
use winapi::um::errhandlingapi::{GetLastError, SetLastError};
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
use winapi::um::userenv::GetUserProfileDirectoryW;
use winapi::um::winnt::{HANDLE, TOKEN_READ};

pub fn home_dir_inner() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(home_dir_crt)
}

#[cfg(not(target_vendor = "uwp"))]
fn home_dir_crt() -> Option<PathBuf> {
    unsafe {
        let me = GetCurrentProcess();
        let mut token = ptr::null_mut();
        if OpenProcessToken(me, TOKEN_READ, &mut token) == 0 {
            return None;
        }
        let rs = get_user_profile_directory(token);
        let _ = CloseHandle(token);
        rs
    }
}

#[cfg(target_vendor = "uwp")]
fn home_dir_crt() -> Option<PathBuf> {
    None
}

// Inspired from rust/src/libstd/sys/windows/mod.rs#L106
fn get_user_profile_directory(token: HANDLE) -> Option<PathBuf> {
    // Start off with a stack buf but then spill over to the heap if we end up
    // needing more space.
    let mut stack_buf = [0u16; MAX_PATH];
    let mut heap_buf = Vec::new();
    let mut n = stack_buf.len() as DWORD;
    let mut buf = &mut stack_buf[..];
    unsafe {
        loop {
            SetLastError(0);
            match GetUserProfileDirectoryW(token, buf.as_mut_ptr(), &mut n) {
                0 => match GetLastError() {
                    ERROR_INSUFFICIENT_BUFFER => {
                        let extra = n as usize - heap_buf.len();
                        heap_buf.reserve(extra);
                        heap_buf.set_len(n as usize);
                        buf = &mut heap_buf[..];
                    }
                    _code => return None,
                },
                _ => {
                    let n = n as usize - 1; // sz includes the null terminator
                    return Some(PathBuf::from(OsString::from_wide(buf.get_unchecked(..n))));
                }
            }
        }
    }
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
