use std::env;
use std::fs;
use std::io::prelude::*;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

static CARGO_INTEGRATION_TEST_DIR : &'static str = "cit";
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;
thread_local!(static TASK_ID: usize = NEXT_ID.fetch_add(1, Ordering::SeqCst));

pub fn root() -> PathBuf {
    env::current_exe().unwrap()
                  .parent().unwrap()
                  .join(CARGO_INTEGRATION_TEST_DIR)
                  .join(&TASK_ID.with(|my_id| format!("test-{}", my_id)))
}

pub fn home() -> PathBuf {
    root().join("home")
}

pub trait CargoPathExt {
    fn rm_rf(&self) -> io::Result<()>;
    fn mkdir_p(&self) -> io::Result<()>;
    fn move_into_the_past(&self) -> io::Result<()>;
}

impl CargoPathExt for Path {
    /* Technically there is a potential race condition, but we don't
     * care all that much for our tests
     */
    fn rm_rf(&self) -> io::Result<()> {
        if self.exists() {
            for file in fs::read_dir(self).unwrap() {
                let file = try!(file).path();

                if file.is_dir() {
                    try!(file.rm_rf());
                } else {
                    // On windows we can't remove a readonly file, and git will
                    // often clone files as readonly. As a result, we have some
                    // special logic to remove readonly files on windows.
                    match fs::remove_file(&file) {
                        Ok(()) => {}
                        Err(ref e) if cfg!(windows) &&
                                      e.kind() == ErrorKind::PermissionDenied => {
                            let mut p = file.metadata().unwrap().permissions();
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
        if self.is_file() {
            try!(time_travel(self));
        } else {
            let target = self.join("target");
            for f in try!(fs::walk_dir(self)) {
                let f = try!(f).path();
                if f.starts_with(&target) { continue }
                if !f.is_file() { continue }
                try!(time_travel(&f));
            }
        }
        return Ok(());

        fn time_travel(path: &Path) -> io::Result<()> {
            let stat = try!(path.metadata());

            let hour = 1000 * 3600;
            let newtime = stat.modified() - hour;

            // Sadly change_file_times has a failure mode where a readonly file
            // cannot have its times changed on windows.
            match fs::set_file_times(path, newtime, newtime) {
                Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {}
                e => return e,
            }
            let mut perms = stat.permissions();
            perms.set_readonly(false);
            try!(fs::set_permissions(path, perms));
            fs::set_file_times(path, newtime, newtime)
        }
    }
}

/// Ensure required test directories exist and are empty
pub fn setup() {
    debug!("path setup; root={}; home={}", root().display(), home().display());
    root().rm_rf().unwrap();
    home().mkdir_p().unwrap();
}
