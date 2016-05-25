use std::env;
use std::cell::Cell;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Once, ONCE_INIT};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

use filetime::{self, FileTime};

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cit";
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;

thread_local!(static TASK_ID: usize = NEXT_ID.fetch_add(1, Ordering::SeqCst));

fn init() {
    static GLOBAL_INIT: Once = ONCE_INIT;
    thread_local!(static LOCAL_INIT: Cell<bool> = Cell::new(false));
    GLOBAL_INIT.call_once(|| {
        global_root().mkdir_p().unwrap();
    });
    LOCAL_INIT.with(|i| {
        if i.get() {
            return
        }
        i.set(true);
        root().rm_rf().unwrap();
        home().mkdir_p().unwrap();
    })
}

fn global_root() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop(); // chop off exe name
    path.pop(); // chop off 'debug'

    // If `cargo test` is run manually then our path looks like
    // `target/debug/foo`, in which case our `path` is already pointing at
    // `target`. If, however, `cargo test --target $target` is used then the
    // output is `target/$target/debug/foo`, so our path is pointing at
    // `target/$target`. Here we conditionally pop the `$target` name.
    if path.file_name().and_then(|s| s.to_str()) != Some("target") {
        path.pop();
    }

    path.join(CARGO_INTEGRATION_TEST_DIR)
}

pub fn root() -> PathBuf {
    init();
    global_root().join(&TASK_ID.with(|my_id| format!("t{}", my_id)))
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
            let newtime = mtime.seconds_relative_to_1970() - 3600;
            let nanos = mtime.nanoseconds();
            let newtime = FileTime::from_seconds_since_1970(newtime, nanos);

            // Sadly change_file_times has a failure mode where a readonly file
            // cannot have its times changed on windows.
            match filetime::set_file_times(path, newtime, newtime) {
                Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {}
                e => return e,
            }
            let mut perms = stat.permissions();
            perms.set_readonly(false);
            try!(fs::set_permissions(path, perms));
            filetime::set_file_times(path, newtime, newtime)
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
