use std::env;
use std::fs;
use std::io::prelude::*;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Once, ONCE_INIT};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

use filetime::FileTime;

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cit";
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;
thread_local!(static TASK_ID: usize = NEXT_ID.fetch_add(1, Ordering::SeqCst));

pub fn root() -> PathBuf {
    env::current_exe().unwrap()
                  .parent().unwrap() // chop off exe name
                  .parent().unwrap() // chop off 'debug'
                  .parent().unwrap() // chop off target
                  .join(CARGO_INTEGRATION_TEST_DIR)
                  .join(&TASK_ID.with(|my_id| format!("t{}", my_id)))
}

pub fn home() -> PathBuf {
    root().join("home")
}

pub trait CargoPathExt {
    fn rm_rf(&self) -> io::Result<()>;
    fn mkdir_p(&self) -> io::Result<()>;
    fn move_into_the_past(&self) -> io::Result<()>;

    // cargo versions of the standard PathExt trait
    fn c_exists(&self) -> bool;
    fn c_is_file(&self) -> bool;
    fn c_is_dir(&self) -> bool;
    fn c_metadata(&self) -> io::Result<fs::Metadata>;
}

impl CargoPathExt for Path {
    /* Technically there is a potential race condition, but we don't
     * care all that much for our tests
     */
    fn rm_rf(&self) -> io::Result<()> {
        if self.c_exists() {
            for file in fs::read_dir(self).unwrap() {
                let file = try!(file).path();

                if file.c_is_dir() {
                    try!(file.rm_rf());
                } else {
                    // On windows we can't remove a readonly file, and git will
                    // often clone files as readonly. As a result, we have some
                    // special logic to remove readonly files on windows.
                    match fs::remove_file(&file) {
                        Ok(()) => {}
                        Err(ref e) if cfg!(windows) &&
                                      e.kind() == ErrorKind::PermissionDenied => {
                            let mut p = file.c_metadata().unwrap().permissions();
                            p.set_readonly(false);
                            fs::set_permissions(&file, p).unwrap();
                            try!(fs::remove_file(&file));
                        }
                        Err(e) => return Err(e)
                    }
                }
            }
            fs::remove_dir(self)
        } else {
            Ok(())
        }
    }

    fn mkdir_p(&self) -> io::Result<()> {
        fs::create_dir_all(self)
    }

    fn move_into_the_past(&self) -> io::Result<()> {
        if self.c_is_file() {
            try!(time_travel(self));
        } else {
            try!(recurse(self, &self.join("target")));
        }
        return Ok(());

        fn recurse(p: &Path, bad: &Path) -> io::Result<()> {
            if p.c_is_file() {
                time_travel(p)
            } else if p.starts_with(bad) {
                Ok(())
            } else {
                for f in try!(fs::read_dir(p)) {
                    let f = try!(f).path();
                    try!(recurse(&f, bad));
                }
                Ok(())
            }
        }

        fn time_travel(path: &Path) -> io::Result<()> {
            let stat = try!(path.c_metadata());

            let mtime = FileTime::from_last_modification_time(&stat);
            let newtime = mtime.seconds() - 3600;

            // Sadly change_file_times has a failure mode where a readonly file
            // cannot have its times changed on windows.
            match set_file_times(path, newtime, newtime) {
                Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {}
                e => return e,
            }
            let mut perms = stat.permissions();
            perms.set_readonly(false);
            try!(fs::set_permissions(path, perms));
            set_file_times(path, newtime, newtime)
        }

        #[cfg(unix)]
        fn set_file_times(p: &Path, atime: u64, mtime: u64) -> io::Result<()> {
            use std::os::unix::prelude::*;
            use std::ffi::CString;
            use libc::{timeval, time_t, c_char, c_int};

            let p = try!(CString::new(p.as_os_str().as_bytes()));
            let atime = timeval { tv_sec: atime as time_t, tv_usec: 0, };
            let mtime = timeval { tv_sec: mtime as time_t, tv_usec: 0, };
            let times = [atime, mtime];
            extern {
                fn utimes(name: *const c_char, times: *const timeval) -> c_int;
            }
            unsafe {
                if utimes(p.as_ptr(), times.as_ptr()) == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            }
        }

        #[cfg(windows)]
        fn set_file_times(p: &Path, atime: u64, mtime: u64) -> io::Result<()> {
            use std::fs::OpenOptions;
            use std::os::windows::prelude::*;
            use winapi::{FILETIME, DWORD};
            use kernel32;

            let f = try!(OpenOptions::new().write(true).open(p));
            let atime = to_filetime(atime);
            let mtime = to_filetime(mtime);
            return unsafe {
                let ret = kernel32::SetFileTime(f.as_raw_handle() as *mut _,
                                                0 as *const _,
                                                &atime, &mtime);
                if ret != 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            };

            fn to_filetime(seconds: u64) -> FILETIME {
                // FILETIME is a count of 100ns intervals, and there are 10^7 of
                // these in a second
                let seconds = seconds * 10_000_000;
                FILETIME {
                    dwLowDateTime: seconds as DWORD,
                    dwHighDateTime: (seconds >> 32) as DWORD,
                }
            }
        }
    }

    fn c_exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }

    fn c_is_file(&self) -> bool {
        fs::metadata(self).map(|m| m.is_file()).unwrap_or(false)
    }

    fn c_is_dir(&self) -> bool {
        fs::metadata(self).map(|m| m.is_dir()).unwrap_or(false)
    }

    fn c_metadata(&self) -> io::Result<fs::Metadata> {
        fs::metadata(self)
    }
}

/// Ensure required test directories exist and are empty
pub fn setup() {
    debug!("path setup; root={}; home={}", root().display(), home().display());
    static INIT: Once = ONCE_INIT;
    INIT.call_once(|| {
        root().parent().unwrap().mkdir_p().unwrap();
    });
    root().rm_rf().unwrap();
    home().mkdir_p().unwrap();
}
