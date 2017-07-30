/// Canonical definitions of `home_dir`, `cargo_home`, and `rustup_home`.
///
/// This provides the definition of `home_dir` used by Cargo and
/// rustup, as well functions to find the correct value of
/// `CARGO_HOME` and `RUSTUP_HOME`.
///
/// The definition of `home_dir` provided by the standard library is
/// incorrect because it considers the `HOME` environment variable on
/// Windows. This causes surprising situations where a Rust program
/// will behave differently depending on whether it is run under a
/// Unix emulation environment like Cygwin or MinGW. Neither Cargo nor
/// rustup use the standard libraries definition - they use the
/// definition here.
///
/// This crate further provides two functions, `cargo_home` and
/// `rustup_home`, which are the canonical way to determine the
/// location that Cargo and rustup store their data.
///
/// See [rust-lang/rust#43321].
///
/// [rust-lang/rust#43321]: https://github.com/rust-lang/rust/issues/43321

#[cfg(windows)]
extern crate scopeguard;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate advapi32;
#[cfg(windows)]
extern crate userenv;

#[cfg(windows)]
use winapi::DWORD;
use std::path::{PathBuf, Path};
use std::io;
use std::env;

/// Returns the path of the current user's home directory if known.
///
/// # Unix
///
/// Returns the value of the 'HOME' environment variable if it is set
/// and not equal to the empty string. Otherwise, it tries to determine the
/// home directory by invoking the `getpwuid_r` function on the UID of the
/// current user.
///
/// # Windows
///
/// Returns the value of the 'USERPROFILE' environment variable if it
/// is set and not equal to the empty string. If both do not exist,
/// [`GetUserProfileDirectory`][msdn] is used to return the
/// appropriate path.
///
/// [msdn]: https://msdn.microsoft.com/en-us/library/windows/desktop/bb762280(v=vs.85).aspx
///
/// # Examples
///
/// ```
/// use std::env;
///
/// match env::home_dir() {
///     Some(path) => println!("{}", path.display()),
///     None => println!("Impossible to get your home dir!"),
/// }
/// ```
pub fn home_dir() -> Option<PathBuf> {
    home_dir_()
}

#[cfg(windows)]
fn home_dir_() -> Option<PathBuf> {
    use std::ptr;
    use kernel32::{GetCurrentProcess, GetLastError, CloseHandle};
    use advapi32::OpenProcessToken;
    use userenv::GetUserProfileDirectoryW;
    use winapi::ERROR_INSUFFICIENT_BUFFER;
    use winapi::winnt::TOKEN_READ;
    use scopeguard;

    ::std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(|| unsafe {
        let me = GetCurrentProcess();
        let mut token = ptr::null_mut();
        if OpenProcessToken(me, TOKEN_READ, &mut token) == 0 {
            return None;
        }
        let _g = scopeguard::guard(token, |h| { let _ = CloseHandle(*h); });
        fill_utf16_buf(|buf, mut sz| {
            match GetUserProfileDirectoryW(token, buf, &mut sz) {
                0 if GetLastError() != ERROR_INSUFFICIENT_BUFFER => 0,
                0 => sz,
                _ => sz - 1, // sz includes the null terminator
            }
        }, os2path).ok()
    })
}

#[cfg(windows)]
fn os2path(s: &[u16]) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    PathBuf::from(OsString::from_wide(s))
}

#[cfg(windows)]
fn fill_utf16_buf<F1, F2, T>(mut f1: F1, f2: F2) -> io::Result<T>
    where F1: FnMut(*mut u16, DWORD) -> DWORD,
          F2: FnOnce(&[u16]) -> T
{
    use kernel32::{GetLastError, SetLastError};
    use winapi::{ERROR_INSUFFICIENT_BUFFER};

    // Start off with a stack buf but then spill over to the heap if we end up
    // needing more space.
    let mut stack_buf = [0u16; 512];
    let mut heap_buf = Vec::new();
    unsafe {
        let mut n = stack_buf.len();
        loop {
            let buf = if n <= stack_buf.len() {
                &mut stack_buf[..]
            } else {
                let extra = n - heap_buf.len();
                heap_buf.reserve(extra);
                heap_buf.set_len(n);
                &mut heap_buf[..]
            };

            // This function is typically called on windows API functions which
            // will return the correct length of the string, but these functions
            // also return the `0` on error. In some cases, however, the
            // returned "correct length" may actually be 0!
            //
            // To handle this case we call `SetLastError` to reset it to 0 and
            // then check it again if we get the "0 error value". If the "last
            // error" is still 0 then we interpret it as a 0 length buffer and
            // not an actual error.
            SetLastError(0);
            let k = match f1(buf.as_mut_ptr(), n as DWORD) {
                0 if GetLastError() == 0 => 0,
                0 => return Err(io::Error::last_os_error()),
                n => n,
            } as usize;
            if k == n && GetLastError() == ERROR_INSUFFICIENT_BUFFER {
                n *= 2;
            } else if k >= n {
                n = k;
            } else {
                return Ok(f2(&buf[..k]))
            }
        }
    }
}

#[cfg(any(unix, target_os = "redox"))]
fn home_dir_() -> Option<PathBuf> {
    ::std::env::home_dir()
}

/// Returns the storage directory used by Cargo, often knowns as
/// `.cargo` or `CARGO_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `CARGO_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `CARGO_HOME` environment variable, if `CARGO_HOME` is a
///   relative directory.
/// - The `.cargo` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
pub fn cargo_home() -> io::Result<PathBuf> {
    let cwd = env::current_dir()?;
    cargo_home_with_cwd(&cwd)
}

pub fn cargo_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    let env_var = env::var_os("CARGO_HOME");

    // NB: During the multirust-rs -> rustup transition the install
    // dir changed from ~/.multirust/bin to ~/.cargo/bin. Because
    // multirust used to explicitly set CARGO_HOME it's possible to
    // get here when e.g. installing under `cargo run` and decide to
    // install to the wrong place. This check is to make the
    // multirust-rs to rustup upgrade seamless.
    let env_var = if let Some(v) = env_var {
       let vv = v.to_string_lossy().to_string();
       if vv.contains(".multirust/cargo") ||
            vv.contains(r".multirust\cargo") ||
            vv.trim().is_empty() {
           None
       } else {
           Some(v)
       }
    } else {
        None
    };

    let env_cargo_home = env_var.map(|home| cwd.join(home));
    let home_dir = home_dir()
        .ok_or(io::Error::new(io::ErrorKind::Other, "couldn't find home dir"));
    let user_home = home_dir.map(|p| p.join(".cargo"));

    // Compatibility with old cargo that used the std definition of home_dir
    let compat_home_dir = ::std::env::home_dir();
    let compat_user_home = compat_home_dir.map(|p| p.join(".cargo"));
    
    if let Some(p) = env_cargo_home {
        Ok(p)
    } else {
        if let Some(d) = compat_user_home {
            if d.exists() {
                Ok(d)
            } else {
                user_home
            }                
        } else {
            user_home
        }
    }
}

/// Returns the storage directory used by rustup, often knowns as
/// `.rustup` or `RUSTUP_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `RUSTUP_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `RUSTUP_HOME` environment variable, if `RUSTUP_HOME` is a
///   relative directory.
/// - The `.rustup` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// As a matter of backwards compatibility, this function _may_ return
/// the `.multirust` directory in the user's home directory, only if
/// it determines that the user is running an old version of rustup
/// where that is necessary.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
pub fn rustup_home() -> io::Result<PathBuf> {
    let cwd = env::current_dir()?;
    rustup_home_with_cwd(&cwd)
}

pub fn rustup_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    let env_var = env::var_os("RUSTUP_HOME");
    let env_rustup_home = env_var.map(|home| cwd.join(home));
    let home_dir = home_dir()
        .ok_or(io::Error::new(io::ErrorKind::Other, "couldn't find home dir"));

    let user_home = if use_rustup_dir() {
        home_dir.map(|d| d.join(".rustup"))
    } else {
        home_dir.map(|d| d.join(".multirust"))
    };

    if let Some(p) = env_rustup_home {
        Ok(p)
    } else {
        user_home
    }
}

fn use_rustup_dir() -> bool {
    fn rustup_dir() -> Option<PathBuf> {
        home_dir().map(|p| p.join(".rustup"))
    }

    fn multirust_dir() -> Option<PathBuf> {
        home_dir().map(|p| p.join(".multirust"))
    }

    fn rustup_dir_exists() -> bool {
        rustup_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn multirust_dir_exists() -> bool {
        multirust_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn rustup_old_version_exists() -> bool {
        rustup_dir()
            .map(|p| p.join("rustup-version").exists())
            .unwrap_or(false)
    }

    !rustup_old_version_exists()
        && (rustup_dir_exists() || !multirust_dir_exists())
}
